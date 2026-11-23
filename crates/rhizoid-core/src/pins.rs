//! Pins: where a follower writes the release of a tool it uses, read and moved in one place - the file and the
//! key or line the graph names, and only the pinned value changed, proven by reading it back.

use crate::graph::{PinRead, RepoName, Tool};
use crate::repos::RepoError;

/// The version a pin file names for `tool`: its key's value (the `value` template stripped) or its line's;
/// `None` when the file names none.
pub fn read_pin(tool: &Tool, follower: &RepoName, text: &str) -> Result<Option<String>, RepoError> {
    match &tool.read {
        PinRead::Key(path) => {
            let table = text.parse::<toml::Table>().map_err(|e| {
                RepoError::Failed(format!("{follower}: {} is not valid TOML: {e}", tool.pin))
            })?;
            let mut v = table.get(&path[0]);
            for k in &path[1..] {
                v = v.and_then(|x| x.get(k));
            }
            match v {
                None => Ok(None),
                Some(toml::Value::String(s)) => {
                    let (before, after) = &tool.value;
                    s.strip_prefix(before.as_str())
                        .and_then(|rest| rest.strip_suffix(after.as_str()))
                        .map(|version| Some(version.to_string()))
                        .ok_or_else(|| {
                            RepoError::Failed(format!(
                                "{follower}: {}: `{}` is {s:?}, not {before}VERSION{after}",
                                tool.pin,
                                path.join(".")
                            ))
                        })
                }
                Some(_) => Err(RepoError::Failed(format!(
                    "{follower}: {}: `{}` is not a string",
                    tool.pin,
                    path.join(".")
                ))),
            }
        }
        PinRead::Line { before, after } => Ok(text.lines().find_map(|l| {
            let inner = l.strip_prefix(before.as_str())?.strip_suffix(after.as_str())?;
            Some(inner.to_string())
        })),
    }
}

/// The pin file's text with `tool`'s version moved from `old` to `new`: only the pinned value changes, the rest
/// of the file as it was, and the edit is read back before it is given.
pub fn edit_pin(tool: &Tool, text: &str, old: &str, new: &str) -> Result<String, String> {
    let follower = RepoName::try_from("pin/edit".to_string()).expect("a placeholder name");
    let (from, to) = match &tool.read {
        PinRead::Key(_) => {
            let (before, after) = &tool.value;
            (format!("{before}{old}{after}"), format!("{before}{new}{after}"))
        }
        PinRead::Line { before, after } => {
            (format!("{before}{old}{after}"), format!("{before}{new}{after}"))
        }
    };
    let edited = match &tool.read {
        PinRead::Key(_) => {
            // the value as TOML writes it, in either kind of quotes; it must appear once
            let quoted = [format!("\"{from}\""), format!("'{from}'")];
            let hits: Vec<usize> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| quoted.iter().any(|q| code_part(l).contains(q.as_str())))
                .map(|(i, _)| i)
                .collect();
            let [at] = hits.as_slice() else {
                return Err(format!(
                    "{} holds {from:?} {} times, not once: move it by hand",
                    tool.pin,
                    hits.len()
                ));
            };
            replace_line(text, *at, |l| {
                l.replacen(&quoted[0], &format!("\"{to}\""), 1).replacen(
                    &quoted[1],
                    &format!("'{to}'"),
                    1,
                )
            })
        }
        PinRead::Line { .. } => {
            let hits: Vec<usize> =
                text.lines().enumerate().filter(|(_, l)| *l == from).map(|(i, _)| i).collect();
            let [at] = hits.as_slice() else {
                return Err(format!(
                    "{} has the line {from:?} {} times, not once: move it by hand",
                    tool.pin,
                    hits.len()
                ));
            };
            replace_line(text, *at, |_| to.clone())
        }
    };
    let read_back = matches!(read_pin(tool, &follower, &edited), Ok(Some(v)) if v == new);
    if !read_back || !only_the_key_moved(tool, text, &edited, &to) {
        return Err(format!(
            "{}: the edit did not read back as {new} alone: move it by hand",
            tool.pin
        ));
    }
    Ok(edited)
}

/// For a key pin, the edited file parses as the original with that one key set to `to`: nothing else moved.
fn only_the_key_moved(tool: &Tool, original: &str, edited: &str, to: &str) -> bool {
    let PinRead::Key(path) = &tool.read else { return true };
    let (Ok(mut want), Ok(got)) = (original.parse::<toml::Table>(), edited.parse::<toml::Table>())
    else {
        return false;
    };
    let mut at: &mut toml::Table = &mut want;
    for k in &path[..path.len() - 1] {
        match at.get_mut(k) {
            Some(toml::Value::Table(t)) => at = t,
            _ => return false,
        }
    }
    at.insert(path[path.len() - 1].clone(), toml::Value::String(to.to_string()));
    want == got
}

/// A TOML line without its comment: cut at the first `#` outside quotes.
fn code_part(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '#') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn replace_line(text: &str, at: usize, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if i == at {
            let (body, end) = match line.strip_suffix("\r\n") {
                Some(b) => (b, "\r\n"),
                None => match line.strip_suffix('\n') {
                    Some(b) => (b, "\n"),
                    None => (line, ""),
                },
            };
            out.push_str(&f(body));
            out.push_str(end);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// What kind of pin: a tool's version, which `bump` moves, or a slice's release, which `follow` moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A tool pinned at a release of its source.
    Tool,
    /// A slice of files taken at a release of its source.
    Slice,
}

/// One pin of the graph: who pins what, where, at which release, and the newest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pin {
    /// The repository that pins.
    pub follower: RepoName,
    /// What it is.
    pub kind: Kind,
    /// The tool's name, or the slice's source.
    pub name: String,
    /// The file that holds the pin.
    pub file: String,
    /// The release it names (`None`: the file names none).
    pub pinned: Option<String>,
    /// The newest release of its source.
    pub latest: String,
    /// The one job that moves it, as its edge names it.
    pub moved_by: String,
    /// For a tool pin: each other place (`FILE:LINE`) its tag name is written, outside its pin file and its `also`
    /// files - a pin bump would move only one copy of.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<String>,
    /// The tool, for a tool pin: where to find it in the graph.
    #[serde(skip)]
    pub tool: Option<usize>,
    /// The slice, for a slice pin: where to find it in the graph.
    #[serde(skip)]
    pub slice: Option<usize>,
}

impl Pin {
    /// Whether it names the newest release.
    pub fn current(&self) -> bool {
        self.pinned.as_deref() == Some(self.latest.as_str())
    }

    /// Whether `rhizoid bump` is the one that moves it.
    pub fn moved_by_bump(&self) -> bool {
        self.moved_by == crate::graph::BUMP_MOVER
    }
}

/// Each other place (`FILE:LINE`) a tool pin's tag name (its prefix and the release it pins) is written in the follower
/// at `rev`, outside the pin file and the files bump commits with it (`also`): one pin, one place (decision
/// 2026-11-23-every-pin-one-pushed-mover, google-walleye-lab #790).
pub fn copies(
    t: &crate::graph::Tool,
    follower: &RepoName,
    pinned: &str,
    repos: &dyn crate::Repos,
    rev: &str,
) -> Result<Vec<String>, RepoError> {
    // a bare version is no tag name: it would be found in every lock file
    if t.tag_prefix.is_empty() {
        return Ok(Vec::new());
    }
    let tag = format!("{}{pinned}", t.tag_prefix);
    let plain = |p: &str| p.strip_prefix("./").unwrap_or(p).to_string();
    let (pin, also): (String, Vec<String>) =
        (plain(&t.pin), t.also.iter().map(|a| plain(a)).collect());
    let hits: Vec<(String, u64)> = repos
        .grep(follower, rev, &tag)?
        .into_iter()
        .filter(|(path, _, text)| *path != pin && !also.contains(path) && names_tag(text, &tag))
        .map(|(path, line, _)| (path, line))
        .collect();
    // a file the follower marks as a record (history that names a version) is no second pin
    let mut files: Vec<String> = hits.iter().map(|(p, _)| p.clone()).collect();
    files.dedup();
    let records = repos.marked(follower, rev, RECORDS, &files)?;
    Ok(hits
        .into_iter()
        .filter(|(path, _)| !records.contains(path))
        .map(|(path, line)| format!("{path}:{line}"))
        .collect())
}

/// The git attribute a follower sets in its own .gitattributes on the files that are records, not config: history
/// that names a version (`data/** rhizoid-records`). pin-copies never counts them.
pub const RECORDS: &str = "rhizoid-records";

/// Whether `text` names the tag `tag` itself, not a longer one that starts or ends with it: `lpoco-v0.17.1` is not
/// in `lpoco-v0.17.10`, `lpoco-v0.17.1-cut.2`, `lpoco-v0.17.1.1` or `xlpoco-v0.17.1`.
fn names_tag(text: &str, tag: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(tag).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let mut after = text[at + tag.len()..].chars();
        let (next, then) = (after.next(), after.next());
        let starts = !before.is_some_and(|c| word(c) || c == '-' || c == '.');
        let ends = match (next, then) {
            (None, _) => true,
            (Some(c), _) if word(c) => false,
            (Some('-' | '.' | '+'), Some(c)) => !word(c),
            _ => true,
        };
        starts && ends
    })
}

#[cfg(test)]
mod copy_tests {
    use super::names_tag;

    #[test]
    fn a_tag_name_is_found_whole_never_inside_a_longer_one() {
        let tag = "lpoco-v0.17.1";
        for yes in [
            "lpoco-v0.17.1",
            "uses: lpoco-v0.17.1\n",
            "\"lpoco-v0.17.1\"",
            "at lpoco-v0.17.1.",
            "(lpoco-v0.17.1)",
        ] {
            assert!(names_tag(yes, tag), "{yes}");
        }
        for no in [
            "lpoco-v0.17.10",
            "lpoco-v0.17.1-cut.2",
            "lpoco-v0.17.1.1",
            "xlpoco-v0.17.1",
            "a-lpoco-v0.17.1",
            "lpoco-v0.17.1+build",
        ] {
            assert!(!names_tag(no, tag), "{no}");
        }
        assert!(names_tag("lpoco-v0.17.10 then lpoco-v0.17.1", tag), "any of its places");
    }
}

/// Every pin of the graph, read at the graph's revision of each follower's clone: its tools' and its slices'.
pub fn plan(graph: &crate::Graph, repos: &dyn crate::Repos) -> Result<Vec<Pin>, RepoError> {
    let rev: &str = &graph.rev;
    let latest = |source: &RepoName, prefix: &str| -> Result<String, RepoError> {
        crate::text::latest_release(&repos.tags(source)?, prefix).ok_or_else(|| {
            RepoError::Failed(format!("{source} has no release tagged {prefix}N.N.N"))
        })
    };
    let mut pins = Vec::new();
    for (i, t) in graph.tools.iter().enumerate() {
        let newest = match &t.tag_suffix {
            Some(s) => crate::text::latest_cut(&repos.tags(&t.source)?, &t.tag_prefix, s)
                .ok_or_else(|| {
                    RepoError::Failed(format!(
                        "{} has no cut tagged {}VERSION{s}",
                        t.source, t.tag_prefix
                    ))
                })?,
            None => latest(&t.source, &t.tag_prefix)?,
        };
        for f in &t.followers {
            let text = repos.file(f, rev, &t.pin)?.unwrap_or_default();
            let pinned = read_pin(t, f, &text)?;
            let copies = match &pinned {
                Some(p) => copies(t, f, p, repos, rev)?,
                None => Vec::new(),
            };
            pins.push(Pin {
                follower: f.clone(),
                kind: Kind::Tool,
                name: t.name.clone(),
                file: t.pin.clone(),
                pinned,
                copies,
                latest: newest.clone(),
                moved_by: t.moved_by.clone(),
                tool: Some(i),
                slice: None,
            });
        }
    }
    for (i, s) in graph.slices.iter().enumerate() {
        let newest = latest(&s.source, &s.tag_prefix)?;
        for f in &s.followers {
            let text = repos.file(f, rev, &s.pin)?.unwrap_or_default();
            let table = text
                .parse::<toml::Table>()
                .map_err(|e| RepoError::Failed(format!("{f}: {} is not valid TOML: {e}", s.pin)))?;
            pins.push(Pin {
                follower: f.clone(),
                kind: Kind::Slice,
                name: s.source.to_string(),
                file: s.pin.clone(),
                pinned: table.get("version").and_then(|v| v.as_str()).map(str::to_string),
                latest: newest.clone(),
                moved_by: s.moved_by.clone(),
                copies: Vec::new(),
                tool: None,
                slice: Some(i),
            });
        }
    }
    Ok(pins)
}

/// How a tool pin's bump ended.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Bumped {
    /// Committed on its branch (and pushed, and its pull request opened, when asked).
    Bumped {
        /// The branch.
        branch: String,
        /// Pushed to `origin`.
        pushed: bool,
        /// What the pull request command printed (its address).
        pr: Option<String>,
        /// The pull request command failed, the branch pushed: open it by hand.
        pr_error: Option<String>,
        /// Landed through the follower's checks (`--land`).
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        landed: bool,
        /// The land command failed - a check or the gate refused: the pull request stays open, and this says why.
        #[serde(skip_serializing_if = "Option::is_none")]
        land_error: Option<String>,
    },
    /// Its branch exists already: an earlier bump, not merged yet.
    BranchExists {
        /// The branch.
        branch: String,
    },
    /// The gate failed: nothing was committed.
    GatedOut {
        /// The gate's exit code.
        exit: Option<i32>,
        /// What it printed.
        output: String,
    },
    /// The edit was not certain (the value twice, or not read back): moved by hand.
    Refused {
        /// Why.
        why: String,
    },
}

/// Bump one tool pin to its newest release in the follower's clone: a branch from the graph's revision, in a
/// fresh worktree; only the pinned value edited and read back; the gate, when the tool has one, passed before
/// the commit; pushed and its pull request opened only when asked. The worktree always goes.
pub fn bump(
    git: &impl crate::GitPort,
    cmd: &impl crate::CommandPort,
    tool: &Tool,
    pin: &Pin,
    at: &BumpAt,
) -> Result<Bumped, crate::PortError> {
    let old = pin.pinned.clone().unwrap_or_default();
    let branch = format!("rhizoid/bump/{}-{}", tool.name, pin.latest);
    on_branch(git, at, pin, &branch, |worktree| {
        let path = worktree.join(&pin.file);
        let io = |e: std::io::Error| crate::PortError(format!("{}: {e}", path.display()));
        let text = std::fs::read_to_string(&path).map_err(io)?;
        let edited = match edit_pin(tool, &text, &old, &pin.latest) {
            Ok(t) => t,
            Err(why) => return Ok(Bumped::Refused { why }),
        };
        std::fs::write(&path, edited).map_err(io)?;
        if let Some(out) = gated(cmd, tool.gate.as_deref(), worktree)? {
            return Ok(out);
        }
        // the commit is the pin and what the tool names in `also`: a gate that changed anything else is refused
        let others: Vec<String> = git
            .changed_files(worktree)?
            .into_iter()
            .filter(|f| *f != pin.file && !tool.also.contains(f))
            .collect();
        if !others.is_empty() {
            return Ok(Bumped::Refused {
                why: format!(
                    "the bump changed more than the pin and its `also`: {}",
                    others.join(", ")
                ),
            });
        }
        let mut files = vec![pin.file.clone()];
        files.extend(tool.also.iter().filter(|f| worktree.join(f).exists()).cloned());
        let message = format!("chore(deps): {} {old} -> {}", tool.name, pin.latest);
        commit_push_pr(git, cmd, at, worktree, &branch, &files, &message)
    })
}

/// Bump one slice pin to its source's newest release in the follower's clone: a branch from the graph's
/// revision, in a fresh worktree, the release laid by `follow` (as idealkb's follow.sh lays it, the pin moved
/// with it); a follower that changed a generic file is left for a hand, as update-pins leaves it; the slice's
/// gate, when it has one, passed; every file follow laid committed. The worktree always goes.
#[allow(clippy::too_many_arguments)]
pub fn bump_slice(
    git: &impl crate::GitPort,
    source_git: &impl crate::GitPort,
    cmd: &impl crate::CommandPort,
    slice: &crate::graph::Slice,
    source: &std::path::Path,
    pin: &Pin,
    at: &BumpAt,
) -> Result<Bumped, crate::PortError> {
    let old = pin.pinned.clone().unwrap_or_else(|| "none".to_string());
    let branch = format!("rhizoid/bump/{}-{}", slice.source.as_str().replace('/', "-"), pin.latest);
    if slice.tag_prefix != "v" {
        return Ok(Bumped::Refused {
            why: format!(
                "follow lays releases tagged vVERSION; {} tags {}VERSION",
                slice.source, slice.tag_prefix
            ),
        });
    }
    // what follow would write must stay in the follower: no absolute or climbing path in the release's list
    let generic = String::from_utf8_lossy(
        &source_git.show(&format!("v{}", pin.latest), "generic")?.unwrap_or_default(),
    )
    .into_owned();
    let escapes = |p: &str| p.starts_with('/') || p.split('/').any(|part| part == "..");
    let named: Vec<&str> = generic
        .lines()
        .flat_map(|l| match l.strip_prefix("#! pin = ").or_else(|| l.strip_prefix("#! from = ")) {
            Some(v) => vec![v],
            None if l.trim_start().starts_with('#') => vec![],
            None => l.split_whitespace().collect(),
        })
        .collect();
    if let Some(bad) = named.iter().find(|p| escapes(p)) {
        return Ok(Bumped::Refused {
            why: format!(
                "{} {}'s generic names {bad}, outside the follower: not laid",
                slice.source, pin.latest
            ),
        });
    }
    on_branch(git, at, pin, &branch, |worktree| {
        let laid = crate::follow::follow(source_git, cmd, source, worktree, &pin.latest)?;
        match laid.code {
            0 => {}
            3 => {
                return Ok(Bumped::Refused {
                    why: format!(
                    "the follower changed a generic file: left for a hand to look at (follow: {})",
                    laid.said.lines().find(|l| l.contains("replaced")).unwrap_or("")
                ),
                })
            }
            code => {
                let why = laid.complained.trim();
                let why = if why.is_empty() { laid.said.lines().last().unwrap_or("") } else { why };
                return Ok(Bumped::Refused { why: format!("follow ended {code}: {why}") });
            }
        }
        // the graph's pin must be the one follow moved
        let moved = std::fs::read_to_string(worktree.join(&slice.pin)).unwrap_or_default();
        if !moved.lines().any(|l| l == format!("version = \"{}\"", pin.latest)) {
            return Ok(Bumped::Refused {
                why: format!(
                    "follow did not move {} to {}: the source's generic names another pin file",
                    slice.pin, pin.latest
                ),
            });
        }
        let files = git.changed_files(worktree)?;
        if let Some(out) = gated(cmd, slice.gate.as_deref(), worktree)? {
            return Ok(out);
        }
        // the commit is what follow laid: a gate that changed more is refused, not swept in
        let after = git.changed_files(worktree)?;
        let more: Vec<&String> = after.iter().filter(|f| !files.contains(f)).collect();
        if !more.is_empty() {
            return Ok(Bumped::Refused {
                why: format!(
                    "the gate changed more than follow laid: {}",
                    more.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ),
            });
        }
        let message = format!("chore(deps): follow {} {old} -> {}", slice.source, pin.latest);
        commit_push_pr(git, cmd, at, worktree, &branch, &files, &message)
    })
}

/// Run a change on a fresh branch of the follower's clone, from the graph's revision, in a worktree of its own:
/// refused when the branch is there already (here or on origin); the branch kept once committed, dropped when the
/// change ends otherwise; the worktree always removed.
fn on_branch(
    git: &impl crate::GitPort,
    at: &BumpAt,
    pin: &Pin,
    branch: &str,
    change: impl FnOnce(&std::path::Path) -> Result<Bumped, crate::PortError>,
) -> Result<Bumped, crate::PortError> {
    let here = git.commit(&format!("refs/heads/{branch}"))?.is_some();
    if here || git.commit(&format!("refs/remotes/origin/{branch}"))?.is_some() {
        return Ok(Bumped::BranchExists { branch: branch.to_string() });
    }
    let base = git.commit(at.rev)?.ok_or_else(|| {
        crate::PortError(format!("{}: no {} in the clone (fetch it)", pin.follower, at.rev))
    })?;
    let worktree = at.home.join(branch.replace('/', "-"));
    git.add_branch_worktree(&worktree, branch, &base)?;
    let outcome = change(&worktree);
    let _ = git.remove_worktree(&worktree);
    match outcome {
        // once landed, the change is on main and the local branch is litter
        Ok(b @ Bumped::Bumped { landed: true, .. }) => {
            let _ = git.delete_branch(branch);
            Ok(b)
        }
        // once committed, the branch is kept: pushed or not, it is the bump
        Ok(b @ Bumped::Bumped { .. }) => Ok(b),
        Ok(other) => {
            git.delete_branch(branch)?;
            Ok(other)
        }
        Err(e) => {
            let _ = git.delete_branch(branch);
            Err(e)
        }
    }
}

/// A gate's verdict: `None` when it passed (or there is none), the gated-out outcome when it did not.
fn gated(
    cmd: &impl crate::CommandPort,
    gate: Option<&[String]>,
    worktree: &std::path::Path,
) -> Result<Option<Bumped>, crate::PortError> {
    let Some(gate) = gate else { return Ok(None) };
    let (exit, stdout, stderr) = cmd.run(gate, worktree)?;
    Ok((exit != Some(0)).then(|| Bumped::GatedOut { exit, output: format!("{stdout}{stderr}") }))
}

/// Commit these files with `message`; push and open the pull request when asked. A pull request that fails
/// after the push keeps the branch and says why.
fn commit_push_pr(
    git: &impl crate::GitPort,
    cmd: &impl crate::CommandPort,
    at: &BumpAt,
    worktree: &std::path::Path,
    branch: &str,
    files: &[String],
    message: &str,
) -> Result<Bumped, crate::PortError> {
    git.commit_files(worktree, files, message)?;
    let pushed = at.push || at.pr.is_some();
    if pushed {
        git.push(worktree, branch)?;
    }
    let (mut pr, mut pr_error) = (None, None);
    if let Some(command) = at.pr {
        let argv: Vec<String> = command.iter().map(|a| a.replace("{branch}", branch)).collect();
        match cmd.run(&argv, worktree) {
            Ok((Some(0), stdout, _)) => pr = Some(stdout.trim().to_string()),
            Ok((_, stdout, stderr)) => {
                pr_error = Some(format!("{stdout}{stderr}").trim().to_string())
            }
            Err(e) => pr_error = Some(e.0),
        }
    }
    let (mut landed, mut land_error) = (false, None);
    if let (Some(command), Some(_)) = (at.land, &pr) {
        match cmd.run(command, worktree) {
            Ok((Some(0), _, _)) => landed = true,
            Ok((exit, stdout, stderr)) => {
                let said = format!("{stdout}{stderr}").trim().to_string();
                let code = exit.map_or("a signal".to_string(), |c| format!("exit {c}"));
                land_error = Some(if said.is_empty() {
                    format!("ended with {code}, saying nothing")
                } else {
                    said
                })
            }
            Err(e) => land_error = Some(e.0),
        }
    }
    Ok(Bumped::Bumped { branch: branch.to_string(), pushed, pr, pr_error, landed, land_error })
}

/// Where a bump happens and how far it goes.
pub struct BumpAt<'a> {
    /// The revision the branch starts from (`origin/main`).
    pub rev: &'a str,
    /// rhizoid's worktree home for the follower's clone.
    pub home: &'a std::path::Path,
    /// Push the branch to `origin`.
    pub push: bool,
    /// The pull request command (argv, `{branch}` in it replaced), run once pushed.
    pub pr: Option<&'a [String]>,
    /// The land command (argv), run in the worktree on the branch once its pull request is open.
    pub land: Option<&'a [String]>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Graph;

    fn tool(edge: &str) -> Tool {
        Graph::parse(&format!("[[tool]]\nname = \"t\"\nsource = \"a/b\"\ntag_prefix = \"t-v\"\nfollowers = [\"c/d\"]\n{edge}"))
            .unwrap()
            .tools
            .remove(0)
    }

    fn name() -> RepoName {
        RepoName::try_from("c/d".to_string()).unwrap()
    }

    #[test]
    fn a_key_pin_moves_and_nothing_else_does() {
        let t = tool("pin = \"lab.toml\"\nkey = \"tools.lpoco\"\n");
        let text = "# keep me\n[tools]\nlpoco = \"0.42.2\" # pinned\nother = \"0.42.2-x\"\n";
        assert_eq!(read_pin(&t, &name(), text).unwrap().as_deref(), Some("0.42.2"));
        let moved = edit_pin(&t, text, "0.42.2", "0.43.0").unwrap();
        assert_eq!(
            moved,
            "# keep me\n[tools]\nlpoco = \"0.43.0\" # pinned\nother = \"0.42.2-x\"\n"
        );
    }

    #[test]
    fn a_cargo_git_tag_pin_keeps_its_prefix() {
        let t = tool("pin = \"Cargo.toml\"\nkey = \"dependencies.lpoco-attended.tag\"\nvalue = \"lpoco-attended-v{version}\"\n");
        let text = "[dependencies]\nlpoco-attended = { git = \"https://x/lab-tools\", tag = \"lpoco-attended-v0.34.1\" }\n";
        assert_eq!(read_pin(&t, &name(), text).unwrap().as_deref(), Some("0.34.1"));
        let moved = edit_pin(&t, text, "0.34.1", "0.41.0").unwrap();
        assert!(moved.contains("tag = \"lpoco-attended-v0.41.0\""), "{moved}");
        assert!(moved.contains("git = \"https://x/lab-tools\""));
    }

    #[test]
    fn a_cargo_patch_entry_with_a_url_in_its_key_moves() {
        let t = tool("pin = \"Cargo.toml\"\nkey = 'patch.\"https://github.com/linux-phone-oco/lab-tools\".lpoco-bringup.tag'\nvalue = \"lpoco-v{version}-cut.1\"\n");
        let text = "[dependencies]\nlpoco-bringup = { git = \"https://github.com/linux-phone-oco/lab-tools\", tag = \"lpoco-v0.42.0\" }\n\n[patch.\"https://github.com/linux-phone-oco/lab-tools\"]\nlpoco-bringup = { git = \"https://github.com/grenudi/lab-tools\", tag = \"lpoco-v0.42.0-cut.1\" }\n";
        assert_eq!(read_pin(&t, &name(), text).unwrap().as_deref(), Some("0.42.0"));
        let moved = edit_pin(&t, text, "0.42.0", "0.43.0").unwrap();
        assert!(moved.contains("tag = \"lpoco-v0.43.0-cut.1\""), "{moved}");
        assert!(
            moved.contains("tag = \"lpoco-v0.42.0\" }"),
            "the upstream dependency itself stays: {moved}"
        );
    }

    #[test]
    fn a_line_pin_moves_its_line() {
        let t = tool("pin = \"bin/install.sh\"\nline = 'version=\"${V:-{version}}\"'\n");
        let text = "#!/bin/sh\nversion=\"${V:-0.7.1}\"\necho $version\n";
        assert_eq!(
            edit_pin(&t, text, "0.7.1", "0.8.0").unwrap(),
            "#!/bin/sh\nversion=\"${V:-0.8.0}\"\necho $version\n"
        );
    }

    #[test]
    fn a_comment_or_crlf_does_not_get_in_the_way() {
        let t = tool("pin = \"lab.toml\"\nkey = \"tools.lpoco\"\n");
        let text = "[tools]\n# was \"0.42.2\" before\r\nlpoco = \"0.42.2\"\r\n";
        assert_eq!(
            edit_pin(&t, text, "0.42.2", "0.43.0").unwrap(),
            "[tools]\n# was \"0.42.2\" before\r\nlpoco = \"0.43.0\"\r\n"
        );
        let line = tool("pin = \"i.sh\"\nline = \"v={version}\"\n");
        assert_eq!(edit_pin(&line, "a\r\nv=1\r\nb\r\n", "1", "2").unwrap(), "a\r\nv=2\r\nb\r\n");
    }

    #[test]
    fn an_edit_that_moves_another_key_too_is_refused() {
        let t = tool("pin = \"C.toml\"\nkey = \"d.x.tag\"\nvalue = \"x-v{version}\"\n");
        let text = "[d]\nx = { tag = \"x-v1\", rev = \"x-v1\" }\n";
        let moved = edit_pin(&t, text, "1", "2").unwrap();
        assert!(
            moved.contains("tag = \"x-v2\"") && moved.contains("rev = \"x-v1\""),
            "only the key: {moved}"
        );
    }

    #[test]
    fn an_edit_that_is_not_certain_is_refused() {
        let t = tool("pin = \"lab.toml\"\nkey = \"tools.lpoco\"\n");
        let twice = "[tools]\nlpoco = \"0.42.2\"\n[other]\nx = \"0.42.2\"\n";
        assert!(edit_pin(&t, twice, "0.42.2", "0.43.0").unwrap_err().contains("2 times"));
        let prefixed = tool("pin = \"C.toml\"\nkey = \"d.x.tag\"\nvalue = \"x-v{version}\"\n");
        assert!(
            read_pin(&prefixed, &name(), "[d]\nx = { tag = \"y-v1\" }\n").is_err(),
            "not the template's shape"
        );
        assert!(Graph::parse("[[tool]]\nname = \"t\"\nsource = \"a/b\"\ntag_prefix = \"v\"\npin = \"p\"\nkey = \"k\"\nvalue = \"no-version\"\nfollowers = []\n").is_err());
    }
}
