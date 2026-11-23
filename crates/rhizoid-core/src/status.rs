//! `status`: every edge of a graph against what its source gives. A function of the graph and of what the
//! [`Repos`] port read, so each case is a fixture and none needs a network.
//!
//! The checks are those of research base-graph's `graph-drift.sh`, which this replaces: a slice's pin and its
//! own files, a layer's pin and owner and tables, a tool's pin and list, a fork's tracking branch. Where it
//! answers differently from the shell it is on purpose, and each place says so: a file named in `own` that
//! neither side has, a pin the source never released, a fork that is ahead and not behind, a pin file that is
//! not what it should be (the shell's `sed` read whatever was there).

use crate::finding::{Check, Finding, Verdict};
use crate::graph::{Fork, Graph, Layer, PinRead, PinTable, RepoName, Slice, Tool};
use crate::repos::{Level, RefRead, RepoError, Repos};
use crate::text::{
    is_three_part_release, latest_release, lines_missing, marked_part, raw_marked_part, same_text,
    table_names,
};
use std::collections::BTreeMap;

/// Every edge's findings, in the graph's order. The first thing the world cannot answer stops it.
pub fn status(graph: &Graph, repos: &dyn Repos) -> Result<Vec<Finding>, RepoError> {
    let cx = Cx { graph, repos };
    let mut out = Vec::new();
    for s in &graph.slices {
        for f in &s.followers {
            cx.slice(s, f, &mut out)?;
        }
    }
    for l in &graph.layers {
        cx.layer(l, &mut out)?;
    }
    for t in &graph.tools {
        for f in &t.followers {
            cx.tool(t, f, &mut out)?;
        }
    }
    for p in &graph.pin_tables {
        cx.pin_table(p, &mut out)?;
    }
    for r in &graph.references {
        out.push(reference(r, repos)?);
    }
    for f in &graph.forks {
        cx.fork(f, &mut out)?;
    }
    Ok(out)
}

struct Cx<'a> {
    graph: &'a Graph,
    repos: &'a dyn Repos,
}

/// A finding of one edge: who follows, from where, which check; the verdict and how are given after.
struct Edge<'a> {
    follower: &'a str,
    source: &'a str,
    check: Check,
}

impl Edge<'_> {
    fn is(&self, verdict: Verdict, detail: String) -> Finding {
        Finding {
            follower: self.follower.to_string(),
            source: self.source.to_string(),
            check: self.check,
            verdict,
            carried_by: None,
            detail,
        }
    }
    fn current(&self, detail: String) -> Finding {
        self.is(Verdict::Current, detail)
    }
    fn behind(&self, detail: String) -> Finding {
        self.is(Verdict::Behind, detail)
    }
    fn by_hand(&self, detail: String) -> Finding {
        self.is(Verdict::MovedByHand, detail)
    }
    fn carried(&self, detail: String, item: String) -> Finding {
        Finding { carried_by: Some(item), ..self.is(Verdict::MovedByHand, detail) }
    }
}

fn parse_toml(repo: &RepoName, path: &str, text: &str) -> Result<toml::Table, RepoError> {
    text.parse::<toml::Table>()
        .map_err(|e| RepoError::Failed(format!("{repo}: {path} is not valid TOML: {e}")))
}

/// A string key of a pin file: absent is `None`; present and not a string is an error, not an absence.
fn pin_string(
    repo: &RepoName,
    path: &str,
    t: &toml::Table,
    key: &str,
) -> Result<Option<String>, RepoError> {
    match t.get(key) {
        None => Ok(None),
        Some(toml::Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(RepoError::Failed(format!("{repo}: {path}: `{key}` is not a string"))),
    }
}

/// What a slice's pin file says: the release followed (if it names one), the files kept as the follower's own,
/// and why each is kept.
struct SlicePin {
    version: Option<String>,
    own: Vec<String>,
    why: BTreeMap<String, String>,
}

/// Why an own file is kept, as its pin file declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why<'a> {
    /// A reason: the follower keeps the file as its own on purpose.
    Declared(&'a str),
    /// `upstream:ITEM`: the change is on its way up.
    Upstream(&'a str),
    /// No reason given.
    None,
}

/// The release a follower pins and its source's newest.
#[derive(Debug, Clone, Copy)]
struct At<'a> {
    version: &'a str,
    latest: &'a str,
}

/// The value of `[why]` that names an upstream item instead of a reason.
const UPSTREAM: &str = "upstream:";

impl SlicePin {
    fn why(&self, file: &str) -> Why<'_> {
        match self.why.get(file).map(|w| w.trim()) {
            None | Some("") => Why::None,
            Some(w) => match w.strip_prefix(UPSTREAM) {
                Some(item) if !item.trim().is_empty() => Why::Upstream(item.trim()),
                Some(_) => Why::None,
                None => Why::Declared(w),
            },
        }
    }
}

fn slice_pin(repo: &RepoName, s: &Slice, text: &str) -> Result<SlicePin, RepoError> {
    let (path, own_key, why_key) = (s.pin.as_str(), s.own_key.as_str(), s.why_key.as_str());
    let t = parse_toml(repo, path, text)?;
    let own = match t.get(own_key) {
        None => Vec::new(),
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                RepoError::Failed(format!(
                    "{repo}: {path}: `{own_key}` holds something that is not a string"
                ))
            })?,
        Some(_) => {
            return Err(RepoError::Failed(format!("{repo}: {path}: `{own_key}` is not a list")))
        }
    };
    let why = match t.get(why_key) {
        None => BTreeMap::new(),
        Some(toml::Value::Table(w)) => w
            .iter()
            .map(|(file, v)| {
                v.as_str().map(|r| (file.clone(), r.to_string())).ok_or_else(|| {
                    RepoError::Failed(format!(
                        "{repo}: {path}: [{why_key}] {file} is not a reason in quotes (a file name with a dot needs quotes too: \"{file}.toml\" = \"...\")"
                    ))
                })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?,
        Some(_) => {
            return Err(RepoError::Failed(format!("{repo}: {path}: `{why_key}` is not a table")))
        }
    };
    Ok(SlicePin { version: pin_string(repo, path, &t, "version")?, own, why })
}

/// Whether `e` says `source` has no such revision: a pin that names a release its source never made.
fn is_no_release(e: &RepoError, source: &RepoName) -> bool {
    matches!(e, RepoError::NoRev { repo, .. } if repo == source.as_str())
}

impl Cx<'_> {
    fn text(&self, repo: &RepoName, rev: &str, path: &str) -> Result<String, RepoError> {
        Ok(self.repos.file(repo, rev, path)?.unwrap_or_default())
    }

    fn latest(&self, repo: &RepoName, prefix: &str) -> Result<String, RepoError> {
        latest_release(&self.repos.tags(repo)?, prefix)
            .ok_or_else(|| RepoError::Failed(format!("{repo} has no release tagged {prefix}N.N.N")))
    }

    fn slice(
        &self,
        s: &Slice,
        follower: &RepoName,
        out: &mut Vec<Finding>,
    ) -> Result<(), RepoError> {
        let rev: &str = &self.graph.rev;
        let edge = |check| Edge { follower: follower.as_str(), source: s.source.as_str(), check };
        let Some(pin_text) = self.repos.file(follower, rev, &s.pin)? else {
            out.push(edge(Check::SlicePin).behind(format!("{follower} has no {} at {rev}", s.pin)));
            return Ok(());
        };
        let pin = slice_pin(follower, s, &pin_text)?;
        let Some(version) = pin.version.clone() else {
            out.push(edge(Check::SlicePin).behind(format!("{} names no version", s.pin)));
            return Ok(());
        };
        let latest = self.latest(&s.source, &s.tag_prefix)?;
        out.push(if version == latest {
            edge(Check::SlicePin).current(format!("{} {version}", s.marker))
        } else {
            edge(Check::SlicePin).behind(format!("pins {} {version}, latest {latest}", s.marker))
        });
        let items = self.items(follower, s)?;
        let at = At { version: &version, latest: &latest };
        for file in &pin.own {
            let check = if *file == s.part_file { Check::OwnPart } else { Check::OwnBehind };
            let found = if check == Check::OwnPart {
                let follow_reads_own = crate::follow::reads_as_own(&pin_text, file);
                self.own_part(s, follower, &version, file, pin.why(file), follow_reads_own)
            } else {
                self.own_file(s, follower, at, file, &items, pin.why(file))
            };
            match found {
                Err(e) if is_no_release(&e, &s.source) => out.push(
                    edge(check)
                        .behind(format!("{file}: the pin {version} is no release of {}", s.source)),
                ),
                other => out.push(other?),
            }
        }
        Ok(())
    }

    /// The follower's upstream items, as (id, the words they are searched for a file name in).
    fn items(&self, follower: &RepoName, s: &Slice) -> Result<Vec<(String, String)>, RepoError> {
        let text = self.text(follower, &self.graph.rev, &s.items)?;
        let mut items = Vec::new();
        for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(line).map_err(|e| {
                RepoError::Failed(format!(
                    "{follower}: {} line {}: not a JSON object: {e}",
                    s.items,
                    n + 1
                ))
            })?;
            let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
            let words = ["id", "title", "note", "local_ref", "file"].map(field).join(" ");
            items.push((field("id"), words));
        }
        Ok(items)
    }

    fn own_file(
        &self,
        s: &Slice,
        follower: &RepoName,
        at: At<'_>,
        file: &str,
        items: &[(String, String)],
        why: Why<'_>,
    ) -> Result<Finding, RepoError> {
        let At { version, latest } = at;
        let edge = Edge {
            follower: follower.as_str(),
            source: s.source.as_str(),
            check: Check::OwnBehind,
        };
        let tag = format!("{}{version}", s.tag_prefix);
        let given = self.repos.file(&s.source, &tag, file)?;
        let kept = self.repos.file(follower, &self.graph.rev, file)?;
        if given.is_none() && kept.is_none() {
            return Ok(edge.behind(format!(
                "{file} is in own, and neither {} {version} nor the follower has it",
                s.source
            )));
        }
        let (given, kept) = (given.unwrap_or_default(), kept.unwrap_or_default());
        if same_text(&given, &kept) {
            return Ok(edge.current(format!("{file} is {}'s at {version}", s.source)));
        }
        if let Why::Declared(reason) = why {
            // as follow says when it moves the pin: a release since the pin that changed the file is worth a look
            let mut look = String::new();
            if latest != version {
                let newer =
                    self.repos.file(&s.source, &format!("{}{latest}", s.tag_prefix), file)?;
                if !same_text(&newer.unwrap_or_default(), &given) {
                    look = format!(
                        "; {} {latest} changed it - look at whether the follower takes it",
                        s.source
                    );
                }
            }
            return Ok(edge.current(format!(
                "{file} differs from {}'s at {version}, declared own: {reason}{look}",
                s.source
            )));
        }
        // upstream:ITEM or no reason: a candidate to go up, or a change nothing explains
        let carried =
            items.iter().find(|(_, words)| words.contains(file)).map(|(id, _)| id.clone());
        let unexplained = if why == Why::None {
            format!(" and no reason in [{}]", s.why_key)
        } else {
            String::new()
        };
        Ok(match carried {
            Some(item) => edge.carried(format!("{file} differs, and an upstream item carries it"), item),
            None => edge.by_hand(format!(
                "{file} differs from {}'s at {version}{unexplained}, and no upstream item names it - neither goes up nor comes down",
                s.source
            )),
        })
    }

    fn own_part(
        &self,
        s: &Slice,
        follower: &RepoName,
        version: &str,
        file: &str,
        why: Why<'_>,
        follow_reads_own: bool,
    ) -> Result<Finding, RepoError> {
        let edge =
            Edge { follower: follower.as_str(), source: s.source.as_str(), check: Check::OwnPart };
        let tag = format!("{}{version}", s.tag_prefix);
        let given = self.text(&s.source, &tag, file)?;
        let kept = self.text(follower, &self.graph.rev, file)?;
        let (want, have) = (marked_part(&given, &s.marker), marked_part(&kept, &s.marker));
        // status agrees with follow: a file follow does not read as own gets its part laid again
        if !follow_reads_own && want != have && !want.is_empty() {
            return Ok(edge.behind(format!(
                "{file}'s {} part is not {} {version}'s, and follow does not read {file} as own (it reads a one-line `own = [...]` in {}): follow again lays it",
                s.marker, s.source, s.pin
            )));
        }
        // follow leaves an own file whole, its marked part too: a part that differs is the follower's, declared
        if let (Why::Declared(reason), true) = (why, want != have || have.is_empty()) {
            let how = if have.is_empty() {
                format!("{file} carries no {} part", s.marker)
            } else if want.is_empty() {
                format!(
                    "{file} carries a {} part that {} {version} does not have",
                    s.marker, s.source
                )
            } else {
                format!(
                    "{file}'s {} part differs in {} lines of {} {version}'s",
                    s.marker,
                    lines_missing(&want, &have),
                    s.source
                )
            };
            return Ok(
                edge.current(format!("{how}, declared own (follow leaves it whole): {reason}"))
            );
        }
        Ok(if have.is_empty() {
            edge.by_hand(format!(
                "{file} carries no {} part; nothing new of {}'s tables reaches it",
                s.marker, s.source
            ))
        } else if want.is_empty() {
            edge.by_hand(format!(
                "{file} carries a {} part that {} {version} does not have",
                s.marker, s.source
            ))
        } else if want != have {
            let n = lines_missing(&want, &have);
            edge.by_hand(format!(
                "{file}'s {} part lacks or differs in {n} lines of {} {version}'s",
                s.marker, s.source
            ))
        } else {
            edge.current(format!("{file}'s {} part is {} {version}'s", s.marker, s.source))
        })
    }

    fn layer(&self, l: &Layer, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let rev: &str = &self.graph.rev;
        let given_text = self.text(&l.owner, rev, &l.part)?;
        let given = table_names(marked_part(&given_text, &l.marker));
        if given.is_empty() {
            let e = Edge {
                follower: l.owner.as_str(),
                source: l.owner.as_str(),
                check: Check::LayerOwner,
            };
            out.push(e.behind(format!(
                "{} gives no layer ({} at {rev} has no {} part)",
                l.owner, l.part, l.marker
            )));
        }
        for follower in &l.followers {
            let Some(pin_text) = self.repos.file(follower, rev, &l.pin)? else {
                let e = Edge {
                    follower: follower.as_str(),
                    source: l.default_source.as_str(),
                    check: Check::LayerPin,
                };
                out.push(e.behind(format!("{follower} has no {} at {rev}", l.pin)));
                continue;
            };
            let pin = parse_toml(follower, &l.pin, &pin_text)?;
            let source = pin_string(follower, &l.pin, &pin, "source")?
                .map(RepoName::try_from)
                .transpose()
                .map_err(|e| RepoError::Failed(format!("{follower}: {}: {e}", l.pin)))?
                .unwrap_or_else(|| l.default_source.clone());
            // the shell took the first of the two lines in file order; a parsed file has no order, and
            // `version` is the word the other pin files use
            let version = match pin_string(follower, &l.pin, &pin, "version")? {
                Some(v) => v,
                None => pin_string(follower, &l.pin, &pin, "template")?.unwrap_or_default(),
            };
            let edge = |check| Edge { follower: follower.as_str(), source: source.as_str(), check };
            out.push(if is_three_part_release(&version) {
                edge(Check::LayerPin).current(format!("{source} {version}"))
            } else {
                edge(Check::LayerPin).behind(format!(
                    "the device layer is pinned as \"{version}\" - a date, no release of {source}; nothing follows it"
                ))
            });
            out.push(if source == l.owner {
                edge(Check::LayerOwner).current(format!("the contract comes from {}, which reads it", l.owner))
            } else {
                edge(Check::LayerOwner).behind(format!(
                    "the contract is declared by {source} and read by {}: one contract, two repositories",
                    l.owner
                ))
            });
            let Some(kept) = self.repos.file(follower, rev, &l.file)? else {
                out.push(
                    edge(Check::LayerFollow)
                        .behind(format!("{follower} has no {} at {rev}", l.file)),
                );
                continue;
            };
            let inside = table_names(marked_part(&kept, &l.marker));
            let outside: Vec<String> = table_names(kept.lines())
                .into_iter()
                .filter(|t| given.contains(t) && !inside.contains(t))
                .collect();
            if outside.is_empty() && !given.is_empty() {
                out.push(edge(Check::LayerFollow).current(format!(
                    "no table of the layer is declared outside the {} markers",
                    l.marker
                )));
            }
            for t in outside {
                out.push(edge(Check::LayerFollow).by_hand(format!(
                    "table {t} is the device layer's, declared outside the {} markers - carried by hand",
                    l.marker
                )));
            }
            if source == l.owner {
                out.push(self.layer_equal(l, follower, &version, &kept)?);
            }
        }
        Ok(())
    }

    /// The follower's marked part against the one the owner gave at the release the follower pins.
    fn layer_equal(
        &self,
        l: &Layer,
        follower: &RepoName,
        version: &str,
        kept: &str,
    ) -> Result<Finding, RepoError> {
        let edge = Edge {
            follower: follower.as_str(),
            source: l.owner.as_str(),
            check: Check::LayerEqual,
        };
        let tag = format!("{}{version}", l.tag_prefix);
        let released = match self.repos.file(&l.owner, &tag, &l.part) {
            Err(e) if is_no_release(&e, &l.owner) => {
                return Ok(edge.behind(format!("the pin {version} is no release of {}", l.owner)))
            }
            other => other?.unwrap_or_default(),
        };
        let want = raw_marked_part(&released, &l.marker);
        if want.is_empty() {
            return Ok(edge.behind(format!("{} {version} gave no {} part", l.owner, l.marker)));
        }
        Ok(if same_text(&want.join("\n"), &raw_marked_part(kept, &l.marker).join("\n")) {
            edge.current(format!("the marked part is {} {version}'s", l.owner))
        } else {
            edge.by_hand(format!(
                "the marked part of {} differs from {} {version}'s",
                l.file, l.owner
            ))
        })
    }

    /// A table of pins covered whole: each key a `[[tool]]` edge's (a `key` at or under it), or unpinned with a why.
    fn pin_table(&self, p: &PinTable, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let rev: &str = &self.graph.rev;
        let edge =
            Edge { follower: p.follower.as_str(), source: "graph", check: Check::PinCoverage };
        let where_ = format!("{} [{}]", p.file, p.table);
        let Some(text) = self.repos.file(&p.follower, rev, &p.file)? else {
            out.push(edge.by_hand(format!("{} has no {} at {rev}", p.follower, p.file)));
            return Ok(());
        };
        let doc = parse_toml(&p.follower, &p.file, &text)?;
        let path = p.path();
        let mut table = Some(&doc);
        for part in &path {
            table = table.and_then(|t| t.get(part)).and_then(|v| v.as_table());
        }
        let Some(table) = table else {
            out.push(edge.by_hand(format!("{where_}: no such table")));
            return Ok(());
        };
        let keys: Vec<&String> = table
            .iter()
            .filter(|(_, v)| !p.git_only || v.as_table().is_some_and(|t| t.contains_key("git")))
            .map(|(k, _)| k)
            .collect();
        let covered_by = |key: &str| {
            self.graph.tools.iter().find(|t| {
                t.pin == p.file
                    && t.followers.contains(&p.follower)
                    && matches!(&t.read, PinRead::Key(k)
                        if k.len() > path.len() && k.starts_with(&path) && k[path.len()] == key)
            })
        };
        let (mut by_edge, mut unpinned, mut neither, mut both) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for key in keys {
            match (covered_by(key), p.unpinned.get(key.as_str())) {
                (Some(t), Some(_)) => both.push(format!("{key} (the {} edge\x27s)", t.name)),
                (Some(t), None) => by_edge.push(format!("{key} ({})", t.name)),
                (None, Some(_)) => unpinned.push(key.to_string()),
                (None, None) => neither.push(key.to_string()),
            }
        }
        let gone: Vec<String> =
            p.unpinned.keys().filter(|k| !table.contains_key(k.as_str())).cloned().collect();
        // every problem said at once, so one fix is not followed by another finding
        let mut wrong = Vec::new();
        if !neither.is_empty() {
            wrong.push(format!(
                "{} pinned and covered by no edge, nor named unpinned with a why - no mover moves it",
                neither.join(", ")
            ));
        }
        if !gone.is_empty() {
            wrong
                .push(format!("unpinned names {}, which the table no longer has", gone.join(", ")));
        }
        if !both.is_empty() {
            wrong.push(format!("unpinned names {}, which an edge covers", both.join(", ")));
        }
        out.push(if !wrong.is_empty() {
            edge.by_hand(format!("{where_}: {}", wrong.join("; ")))
        } else {
            let mut parts = Vec::new();
            if !by_edge.is_empty() {
                parts.push(format!("by an edge: {}", by_edge.join(", ")));
            }
            if !unpinned.is_empty() {
                parts.push(format!("unpinned: {}", unpinned.join(", ")));
            }
            if parts.is_empty() {
                parts.push("the table is empty".to_string());
            }
            edge.current(format!("{where_}: every key covered - {}", parts.join("; ")))
        });
        Ok(())
    }

    fn tool(&self, t: &Tool, follower: &RepoName, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let rev: &str = &self.graph.rev;
        let edge = |check| Edge { follower: follower.as_str(), source: t.source.as_str(), check };
        let Some(text) = self.repos.file(follower, rev, &t.pin)? else {
            out.push(edge(Check::ToolPin).behind(format!("{follower} has no {} at {rev}", t.pin)));
            return Ok(());
        };
        let pinned = crate::pins::read_pin(t, follower, &text)?;
        let latest = match &t.tag_suffix {
            Some(s) => crate::text::latest_cut(&self.repos.tags(&t.source)?, &t.tag_prefix, s)
                .ok_or_else(|| {
                    RepoError::Failed(format!(
                        "{} has no cut tagged {}VERSION{s}",
                        t.source, t.tag_prefix
                    ))
                })?,
            None => self.latest(&t.source, &t.tag_prefix)?,
        };
        let mut one_place = None;
        if let Some(p) = &pinned {
            let copies = crate::pins::copies(t, follower, p, self.repos, rev)?;
            let tag = format!("{}{p}", t.tag_prefix);
            one_place = Some(if copies.is_empty() {
                edge(Check::PinCopies).current(format!("{tag} is written only in {}", t.pin))
            } else {
                let shown: Vec<&str> = copies.iter().take(10).map(String::as_str).collect();
                let more = copies.len().saturating_sub(shown.len());
                let more = if more > 0 { format!(" and {more} more") } else { String::new() };
                let mover = if t.moved_by == crate::graph::BUMP_MOVER {
                    "bump refuses to move one copy of several".to_string()
                } else {
                    format!("{} would move one copy of several", t.moved_by)
                };
                edge(Check::PinCopies).by_hand(format!(
                    "{tag} is also written in {}{more}: {mover}",
                    shown.join(", ")
                ))
            });
        }
        out.push(match pinned {
            Some(p) if p == latest => edge(Check::ToolPin).current(format!("{} {p}", t.name)),
            Some(p) => edge(Check::ToolPin).behind(format!("pins {} {p}, latest {latest}", t.name)),
            None => edge(Check::ToolPin)
                .behind(format!("{} names no {} version; latest {latest}", t.pin, t.name)),
        });
        out.extend(one_place);
        if let Some(l) = &t.listed_in {
            let list = self.repos.file(&l.repo, rev, &l.file)?.ok_or_else(|| {
                RepoError::Failed(format!(
                    "{} has no {} at {rev}: the list of repositories whose {} pin a job moves",
                    l.repo, l.file, t.name
                ))
            })?;
            let listed = list
                .lines()
                .filter(|x| !x.trim().is_empty() && !x.trim_start().starts_with('#'))
                .any(|x| x == follower.as_str());
            let e = Edge {
                follower: follower.as_str(),
                source: l.repo.as_str(),
                check: Check::ToolListed,
            };
            out.push(if listed {
                e.current(format!("{} lists it: a job moves its {} pin", l.file, t.name))
            } else {
                e.by_hand(format!(
                    "{}'s {} does not list it - its {} pin is moved by hand",
                    l.repo, l.file, t.name
                ))
            });
        }
        Ok(())
    }

    fn fork(&self, f: &Fork, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let edge = Edge {
            follower: &f.fork.to_string(),
            source: &f.upstream.to_string(),
            check: Check::ForkTrack,
        };
        let mine = self.repos.head(&f.fork, f.branch.as_deref())?;
        let theirs = self.repos.head(&f.upstream, f.upstream_branch.as_deref())?;
        let branch = f.branch.as_deref().unwrap_or("HEAD");
        let upstream_branch = f.upstream_branch.as_deref().unwrap_or("HEAD");
        out.push(if mine == theirs {
            edge.current(format!("{branch} is {}'s", f.upstream))
        } else {
            match self.repos.level(&f.fork, &mine, &theirs) {
                Level::Behind(n) => {
                    edge.behind(format!("{branch} is {n} commits behind {}'s {upstream_branch}", f.upstream))
                }
                Level::Ahead(n) => edge.by_hand(format!(
                    "{branch} is {n} commits ahead of {}'s {upstream_branch}: the tracking branch was changed by hand",
                    f.upstream
                )),
                Level::Diverged { behind, ahead } => edge.by_hand(format!(
                    "{branch} has diverged from {}'s {upstream_branch}: {behind} commits behind, {ahead} ahead",
                    f.upstream
                )),
                Level::Unknown(why) => edge.behind(format!(
                    "{branch} is not {}'s {upstream_branch}, and how far apart is not known ({why})",
                    f.upstream
                )),
            }
        });
        Ok(())
    }
}

/// A reference's local tag against the commit the graph records: current when it peels to it; behind when it is
/// missing, naming the fetch that makes it; moved by hand when it holds any other commit.
fn reference(r: &crate::graph::Reference, repos: &dyn Repos) -> Result<Finding, RepoError> {
    let edge = Edge { follower: r.repo.as_str(), source: r.url.as_str(), check: Check::Reference };
    let short = &r.commit[..12];
    Ok(match repos.reference(r)? {
        RefRead::At(c) if c == r.commit => {
            edge.current(format!("{} is {} at {} ({short})", r.tag, r.name, r.reference))
        }
        RefRead::At(c) => edge.by_hand(format!(
            "{} is {}, not {short}, the commit the graph records for {} at {}",
            r.tag,
            &c[..c.len().min(12)],
            r.name,
            r.reference
        )),
        RefRead::Missing => edge.behind(format!(
            "{} is missing - in the clone of {}: {} (or status --fetch)",
            r.tag,
            r.repo,
            r.fetch_command()
        )),
        RefRead::Made => {
            edge.current(format!("{} made now: {} at {} ({short})", r.tag, r.name, r.reference))
        }
        RefRead::NotMade(why) => edge.behind(format!(
            "{} is missing and --fetch could not make it: {why} - by hand: {}",
            r.tag,
            r.fetch_command()
        )),
    })
}
