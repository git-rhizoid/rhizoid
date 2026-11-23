//! Kernel edges for `status`: one kernel clone, read against the branch and tag scheme its graph names, the
//! upstream releases it carries, and what `rhizoid assemble` made in it.
//!
//! The checks the structure review asked for (Steward, 2026-11-23): a branch or tag outside the scheme (test
//! kernels are `int/` tags, so an `int/` branch is outside it), a topic not on the newest release, a local topic
//! that is not `origin`'s, an int tag's recipe read back from its message (a branch moved since, or no recipe:
//! not made by assemble), rhizoid's worktrees and their tags. What the clone cannot say is `not-known-yet`.
//! A fork and its upstream are a `[[fork]]` edge.

use crate::assemble::{assembled_for, recipe_of};
use crate::finding::{Check, Finding, Verdict};
use crate::graph::RepoName;
use crate::repos::RepoError;
use crate::text::release_number;
use serde::Deserialize;
use std::collections::BTreeMap;

/// A kernel clone of the graph: its scheme and its upstream releases.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kernel {
    /// The clone, under the root (where `[clones]` says, else `owner/repo`).
    pub clone: RepoName,
    /// What an upstream release tag starts with (`v`, as in `v6.18` and `v6.18-rc3`).
    #[serde(default = "default_release_prefix")]
    pub release_prefix: String,
    /// The names its branches and tags may have.
    #[serde(default)]
    pub scheme: Scheme,
}

fn default_release_prefix() -> String {
    "v".to_string()
}

/// The names a kernel clone's branches and tags may have, as patterns: `/`-separated parts, each a literal
/// with an optional CAPITAL placeholder at its end - `N` stands for digits, any other for one part's text
/// (`topic/ITEM`, `int/DEVICE/LIST/BASE/N`, `topic/ITEM/vN`). Release tags are always in it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheme {
    /// The branch names.
    #[serde(default = "default_branches")]
    pub branches: Vec<Pattern>,
    /// The tag names, beside the release tags.
    #[serde(default = "default_tags")]
    pub tags: Vec<Pattern>,
}

fn patterns(texts: &[&str]) -> Vec<Pattern> {
    texts
        .iter()
        .map(|s| Pattern::try_from(s.to_string()).expect("the default scheme parses"))
        .collect()
}

fn default_branches() -> Vec<Pattern> {
    patterns(&["main", "master", "topic/ITEM", "backport/ITEM/BASE", "wip/LANE/WHAT"])
}

fn default_tags() -> Vec<Pattern> {
    patterns(&["topic/ITEM/vN", "int/DEVICE/LIST/BASE/N", "archive/NAME"])
}

impl Default for Scheme {
    /// The scheme of linux-phone-oco's structure review (decision 2026-11-23-structure-kernel-topics-and-integration,
    /// as amended: test kernels are `int/` tags, never branches), with the clone's `main` and `master`.
    fn default() -> Self {
        Scheme { branches: default_branches(), tags: default_tags() }
    }
}

/// One name pattern of a scheme.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Pattern {
    text: String,
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Part {
    literal: String,
    hole: Hole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hole {
    None,
    Digits,
    Text,
}

impl TryFrom<String> for Pattern {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        let mut parts = Vec::new();
        for part in text.split('/') {
            let at = part.find(|c: char| c.is_ascii_uppercase()).unwrap_or(part.len());
            let (literal, hole) = part.split_at(at);
            let hole = match hole {
                "" => Hole::None,
                "N" => Hole::Digits,
                h if h.chars().all(|c| c.is_ascii_uppercase() || c == '_') => Hole::Text,
                _ => {
                    return Err(format!("`{text}`: a placeholder is CAPITALS at the end of a part"))
                }
            };
            if literal.is_empty() && hole == Hole::None {
                return Err(format!("`{text}` has an empty part"));
            }
            parts.push(Part { literal: literal.to_string(), hole });
        }
        Ok(Pattern { text, parts })
    }
}

impl Pattern {
    /// Whether `name` has this pattern's shape.
    pub fn matches(&self, name: &str) -> bool {
        let names: Vec<&str> = name.split('/').collect();
        names.len() == self.parts.len()
            && self.parts.iter().zip(names).all(|(p, n)| match n.strip_prefix(p.literal.as_str()) {
                None => false,
                Some(rest) => match p.hole {
                    Hole::None => rest.is_empty(),
                    Hole::Digits => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
                    Hole::Text => !rest.is_empty(),
                },
            })
    }
}

/// A worktree of a clone, as git lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// Its folder.
    pub path: String,
    /// The commit it is at.
    pub head: Option<String>,
    /// Why it is locked, when it is.
    pub lock_reason: Option<String>,
    /// Whether it has no change beyond its commit; `None` when its folder could not be read.
    pub clean: Option<bool>,
}

/// What the kernel checks read from a clone; the real reader drives git (rhizoid-cli).
pub trait KernelReads {
    /// The refs under `namespace` (`refs/heads/`), without it, each with the commit it names (a tag peeled).
    fn refs(&self, repo: &RepoName, namespace: &str) -> Result<Vec<(String, String)>, RepoError>;
    /// Whether `a` is an ancestor of `b` (or the same commit).
    fn is_ancestor(&self, repo: &RepoName, a: &str, b: &str) -> Result<bool, RepoError>;
    /// How many commits `to` has that `from` lacks.
    fn count(&self, repo: &RepoName, from: &str, to: &str) -> Result<u64, RepoError>;
    /// The nearest tag under `rev` whose name starts with `prefix` and a digit.
    fn nearest_release(
        &self,
        repo: &RepoName,
        rev: &str,
        prefix: &str,
    ) -> Result<Option<String>, RepoError>;
    /// An annotated tag's message; empty for a lightweight tag (not its commit's message).
    fn tag_message(&self, repo: &RepoName, tag: &str) -> Result<String, RepoError>;
    /// Whether the trees of `a` and `b` are the same.
    fn tree_equal(&self, repo: &RepoName, a: &str, b: &str) -> Result<bool, RepoError>;
    /// The clone's worktrees.
    fn worktrees(&self, repo: &RepoName) -> Result<Vec<Worktree>, RepoError>;
}

/// Every kernel clone's findings, in the graph's order.
pub fn kernel_status(kernels: &[Kernel], git: &dyn KernelReads) -> Result<Vec<Finding>, RepoError> {
    let mut out = Vec::new();
    for k in kernels {
        let clone = KernelClone::read(k, git)?;
        clone.scheme(&mut out);
        clone.topics(&mut out)?;
        clone.branches_against_origin(&mut out)?;
        clone.int_tags(&mut out)?;
        clone.worktrees(&mut out)?;
    }
    Ok(out)
}

/// One kernel clone, its refs read once.
struct KernelClone<'a> {
    kernel: &'a Kernel,
    git: &'a dyn KernelReads,
    heads: BTreeMap<String, String>,
    origin: BTreeMap<String, String>,
    tags: BTreeMap<String, String>,
}

impl<'a> KernelClone<'a> {
    fn read(kernel: &'a Kernel, git: &'a dyn KernelReads) -> Result<Self, RepoError> {
        let refs = |ns: &str| -> Result<BTreeMap<String, String>, RepoError> {
            Ok(git.refs(&kernel.clone, ns)?.into_iter().collect())
        };
        Ok(KernelClone {
            kernel,
            git,
            heads: refs("refs/heads/")?,
            origin: refs("refs/remotes/origin/")?,
            tags: refs("refs/tags/")?,
        })
    }

    fn repo(&self) -> &RepoName {
        &self.kernel.clone
    }

    fn finding(&self, check: Check, source: &str, verdict: Verdict, detail: String) -> Finding {
        Finding {
            follower: self.repo().to_string(),
            source: source.to_string(),
            check,
            verdict,
            carried_by: None,
            detail,
        }
    }

    fn is_release(&self, tag: &str) -> bool {
        tag.strip_prefix(self.kernel.release_prefix.as_str())
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    }

    /// The newest release tag: the highest version; release candidates (`-rcN`) do not count, a stable point
    /// release (`v6.18.5`) does.
    fn newest_release(&self) -> Option<&str> {
        self.tags
            .keys()
            .filter(|t| self.is_release(t) && !t.contains("-rc"))
            .max_by_key(|t| release_order(t, &self.kernel.release_prefix))
            .map(String::as_str)
    }

    fn scheme(&self, out: &mut Vec<Finding>) {
        let scheme = &self.kernel.scheme;
        let stray_branches: Vec<&String> =
            self.heads.keys().filter(|b| !scheme.branches.iter().any(|p| p.matches(b))).collect();
        let stray_tags: Vec<&String> = self
            .tags
            .keys()
            .filter(|t| !self.is_release(t) && !scheme.tags.iter().any(|p| p.matches(t)))
            .collect();
        let stray = |kind: &str, names: &[&String]| {
            let shown: Vec<&str> = names.iter().take(STRAYS_SHOWN).map(|n| n.as_str()).collect();
            let more = match names.len().saturating_sub(STRAYS_SHOWN) {
                0 => String::new(),
                k => format!(" and {k} more"),
            };
            self.finding(
                Check::KernelScheme,
                "scheme",
                Verdict::MovedByHand,
                format!("{} {kind} outside the scheme: {}{more}", names.len(), shown.join(", ")),
            )
        };
        if !stray_branches.is_empty() {
            out.push(stray("branches", &stray_branches));
        }
        if !stray_tags.is_empty() {
            out.push(stray("tags", &stray_tags));
        }
        if stray_branches.is_empty() && stray_tags.is_empty() {
            out.push(self.finding(
                Check::KernelScheme,
                "scheme",
                Verdict::Current,
                format!(
                    "{} branches and {} tags, all in the scheme",
                    self.heads.len(),
                    self.tags.len()
                ),
            ));
        }
    }

    /// The branches a list may name: `topic/ITEM` and `backport/ITEM/BASE`, with their commits.
    fn listable(&self) -> impl Iterator<Item = (&String, &String)> {
        self.heads.iter().filter(|(b, _)| {
            let parts: Vec<&str> = b.split('/').collect();
            matches!(parts.as_slice(), ["topic", _] | ["backport", _, _])
        })
    }

    fn topics(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let newest = self.newest_release();
        for (branch, sha) in self.listable() {
            out.push(match branch.split('/').collect::<Vec<_>>().as_slice() {
                ["backport", _, base] => self.backport(branch, sha, base)?,
                _ => self.topic(branch, sha, newest)?,
            });
        }
        Ok(())
    }

    fn topic(&self, branch: &str, sha: &str, newest: Option<&str>) -> Result<Finding, RepoError> {
        let Some(newest) = newest else {
            return Ok(self.finding(
                Check::TopicBase,
                "upstream",
                Verdict::NotKnownYet,
                format!("{branch}: the clone has no release tag to read it against"),
            ));
        };
        let newest_sha = &self.tags[newest];
        if self.git.is_ancestor(self.repo(), newest_sha, sha)? {
            return Ok(self.finding(
                Check::TopicBase,
                newest,
                Verdict::Current,
                format!("{branch} is on {newest}, the newest release"),
            ));
        }
        Ok(match self.git.nearest_release(self.repo(), sha, &self.kernel.release_prefix)? {
            Some(base) => self.finding(
                Check::TopicBase,
                newest,
                Verdict::Behind,
                format!("{branch} is on {base}; the newest release is {newest}"),
            ),
            None => self.finding(
                Check::TopicBase,
                newest,
                Verdict::NotKnownYet,
                format!("{branch} has no release under it (a fork commit?); the newest release is {newest}"),
            ),
        })
    }

    fn backport(&self, branch: &str, sha: &str, base: &str) -> Result<Finding, RepoError> {
        let found =
            self.tags.get(base).or_else(|| self.heads.get(base)).or_else(|| self.origin.get(base));
        let Some(base_sha) = found else {
            return Ok(self.finding(
                Check::TopicBase,
                base,
                Verdict::NotKnownYet,
                format!("{branch}: the clone has no {base} to read it against"),
            ));
        };
        Ok(if self.git.is_ancestor(self.repo(), base_sha, sha)? {
            self.finding(
                Check::TopicBase,
                base,
                Verdict::Current,
                format!("{branch} is on {base}, as its name says"),
            )
        } else {
            self.finding(
                Check::TopicBase,
                base,
                Verdict::MovedByHand,
                format!("{branch} is not on {base}, which its name says"),
            )
        })
    }

    /// A listable branch whose local commit is not `origin`'s: assemble takes the local one, so it must be said.
    fn branches_against_origin(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let mut equal = 0;
        for (branch, local) in self.listable() {
            let Some(theirs) = self.origin.get(branch) else { continue };
            if theirs == local {
                equal += 1;
                continue;
            }
            let ahead = self.git.count(self.repo(), theirs, local)?;
            let behind = self.git.count(self.repo(), local, theirs)?;
            let source = format!("origin/{branch}");
            out.push(match (ahead, behind) {
                (0, n) => self.finding(Check::BranchOrigin, &source, Verdict::Behind, format!("{branch} is {n} commits behind origin's: assemble would take the stale local one")),
                (n, 0) => self.finding(Check::BranchOrigin, &source, Verdict::MovedByHand, format!("{branch} has {n} commits origin does not")),
                (a, b) => self.finding(Check::BranchOrigin, &source, Verdict::MovedByHand, format!("{branch} has diverged from origin's: {a} commits ahead, {b} behind")),
            });
        }
        if equal > 0 {
            out.push(self.finding(
                Check::BranchOrigin,
                "origin",
                Verdict::Current,
                format!("{equal} topic and backport branches are origin's"),
            ));
        }
        Ok(())
    }

    /// The newest int tag of each `int/DEVICE/LIST/BASE`: its recipe read back, each branch against it.
    fn int_tags(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let mut newest: BTreeMap<&str, (u64, &String)> = BTreeMap::new();
        for tag in self.tags.keys().filter(|t| t.starts_with("int/")) {
            let Some((stem, n)) = tag.rsplit_once('/') else { continue };
            let Ok(n) = n.parse::<u64>() else { continue };
            if newest.get(stem).map_or(true, |(best, _)| n > *best) {
                newest.insert(stem, (n, tag));
            }
        }
        for (_, tag) in newest.values() {
            out.push(self.recipe(tag)?);
        }
        Ok(())
    }

    fn recipe(&self, tag: &str) -> Result<Finding, RepoError> {
        let Some(recipe) = recipe_of(&self.git.tag_message(self.repo(), tag)?) else {
            return Ok(self.finding(
                Check::IntRecipe,
                tag,
                Verdict::MovedByHand,
                format!("{tag} was not made by rhizoid assemble: no recipe in it"),
            ));
        };
        let mut moved = Vec::new();
        let mut i = 1;
        while let (Some(branch), Some(sha)) =
            (recipe.get(&format!("entry.{i}.branch")), recipe.get(&format!("entry.{i}.sha")))
        {
            match self.heads.get(branch).or_else(|| self.origin.get(branch)) {
                Some(now) if now == sha => {}
                Some(_) => moved.push(format!("{branch} moved")),
                None => moved.push(format!("{branch} is gone")),
            }
            i += 1;
        }
        let base = recipe.get("base").map_or("?", String::as_str);
        if i == 1 {
            return Ok(self.finding(
                Check::IntRecipe,
                tag,
                Verdict::MovedByHand,
                format!("{tag}'s recipe names no entry: not as rhizoid assemble writes one"),
            ));
        }
        Ok(if moved.is_empty() {
            self.finding(
                Check::IntRecipe,
                tag,
                Verdict::Current,
                format!("{} entries on {base}; every branch is where the tag took it", i - 1),
            )
        } else {
            self.finding(
                Check::IntRecipe,
                tag,
                Verdict::Behind,
                format!("{} since it was assembled: assemble again", moved.join(", ")),
            )
        })
    }

    /// rhizoid's worktrees: each locked one against its tag; an unlocked one in a `rhizoid` home was left at a
    /// conflict.
    fn worktrees(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        for w in self.git.worktrees(self.repo())? {
            let tag = w.lock_reason.as_deref().and_then(assembled_for);
            let in_home = std::path::Path::new(&w.path)
                .parent()
                .and_then(|p| p.file_name())
                .is_some_and(|n| n == "rhizoid");
            let finding = match tag {
                None if in_home && w.lock_reason.is_none() => self.finding(
                    Check::WorktreeTag,
                    "rhizoid assemble",
                    Verdict::Behind,
                    format!("{}: unlocked, left at a conflict - resolve and commit there, assemble again, then git worktree remove it", w.path),
                ),
                None => continue,
                Some(tag) => self.locked(&w, tag)?,
            };
            out.push(finding);
        }
        Ok(())
    }

    fn locked(&self, w: &Worktree, tag: &str) -> Result<Finding, RepoError> {
        let at = |verdict, how: String| {
            self.finding(Check::WorktreeTag, tag, verdict, format!("{}: {how}", w.path))
        };
        Ok(match (self.tags.get(tag), w.head.as_deref(), w.clean) {
            (None, _, _) => {
                at(Verdict::MovedByHand, format!("locked for {tag}, which was never made"))
            }
            (Some(_), None, _) | (Some(_), _, None) => {
                at(Verdict::NotKnownYet, "its folder could not be read".to_string())
            }
            (Some(_), Some(head), Some(clean)) => {
                if !self.git.tree_equal(self.repo(), head, tag)? {
                    at(
                        Verdict::MovedByHand,
                        format!("locked, but its commit's tree is not {tag}'s"),
                    )
                } else if !clean {
                    at(Verdict::MovedByHand, format!("locked, changed beyond {tag}"))
                } else {
                    at(
                        Verdict::Current,
                        format!("locked, at {tag} (rhizoid assemble --done {tag} ends it)"),
                    )
                }
            }
        })
    }
}

/// How many stray names one finding shows; the rest are counted.
const STRAYS_SHOWN: usize = 10;

/// A release's place in order: its numbers, then a release above its candidates (`-rcN`).
fn release_order(tag: &str, prefix: &str) -> (Vec<u64>, bool, u64) {
    let version = tag.strip_prefix(prefix).unwrap_or(tag);
    let (main, suffix) = version.split_once('-').unwrap_or((version, ""));
    let numbers = release_number(&format!("{prefix}{main}"), prefix).unwrap_or_default();
    let rc = suffix.strip_prefix("rc").and_then(|n| n.parse().ok());
    (numbers, rc.is_none(), rc.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Pattern {
        Pattern::try_from(s.to_string()).unwrap()
    }

    #[test]
    fn a_pattern_matches_its_shape_only() {
        assert!(p("topic/ITEM").matches("topic/panel"));
        assert!(!p("topic/ITEM").matches("topic/panel/v2"));
        assert!(!p("topic/ITEM").matches("topic/"));
        assert!(p("topic/ITEM/vN").matches("topic/panel/v12"));
        assert!(!p("topic/ITEM/vN").matches("topic/panel/vx"));
        assert!(p("int/DEVICE/LIST/BASE/N").matches("int/walleye/ufs-off/v6.18/3"));
        assert!(!p("int/DEVICE/LIST/BASE/N").matches("int/walleye/ufs-off/v6.18/x"));
        assert!(p("main").matches("main") && !p("main").matches("master"));
        assert!(Pattern::try_from("topic/ITEMx".to_string()).is_err());
        assert!(Pattern::try_from("topic//x".to_string()).is_err());
    }

    #[test]
    fn releases_order_by_number_and_a_release_above_its_candidates() {
        let o = |t: &str| release_order(t, "v");
        assert!(o("v6.18") > o("v6.18-rc7"));
        assert!(o("v6.18-rc7") > o("v6.18-rc3"));
        assert!(o("v6.18-rc1") > o("v6.9"));
        assert!(o("v6.10") > o("v6.9"));
    }

    #[test]
    fn the_lock_reason_names_the_tag() {
        assert_eq!(
            assembled_for("rhizoid assemble int/d/l/b/2: wanted until unlocked"),
            Some("int/d/l/b/2")
        );
        assert_eq!(assembled_for("someone else's"), None);
    }
}

#[cfg(test)]
mod example {
    use crate::Graph;

    #[test]
    fn the_example_kernel_graph_reads_and_its_scheme_is_the_default() {
        let g = Graph::parse(include_str!("../../../examples/kernel.graph.toml")).unwrap();
        assert_eq!(g.kernels.len(), 1);
        assert_eq!(g.kernels[0].scheme, super::Scheme::default());
        let only_tags =
            Graph::parse("[[kernel]]\nclone = \"a/b\"\nscheme = { tags = [\"keep/NAME\"] }")
                .unwrap();
        assert_eq!(only_tags.kernels[0].scheme.branches, super::Scheme::default().branches);
        assert!(Graph::parse(
            "[[kernel]]\nclone = \"a/b\"\nscheme = { branches = [\"x/Y\"], tags = [\"bad//\"] }"
        )
        .unwrap_err()
        .contains("empty part"));
    }
}
