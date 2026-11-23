//! Integration edges for `status`: one repository whose branches are merged into tags by `rhizoid assemble`,
//! read against what its graph says - the names its branches and tags may have, the branches lists merge
//! (each on the newest release, or on the base its name says, and equal to `origin`'s), the tags assemble made
//! (each recipe read back from its message), and assemble's worktrees.
//!
//! Every name here is data the graph gives; no project's scheme is in code (decision
//! 2026-11-23-rhizoid-stays-general). What the clone cannot say is `not-known-yet`, never a verdict.

use crate::assemble::{assembled_for, recipe_of};
use crate::finding::{Check, Finding, Verdict};
use crate::graph::RepoName;
use crate::pattern::Pattern;
use crate::repos::RepoError;
use crate::text::release_number;
use serde::Deserialize;
use std::collections::BTreeMap;

/// A repository of the graph whose branches are assembled into tags.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Integration {
    /// The repository, its clone under the root (where `[clones]` says, else `owner/repo`).
    pub repo: RepoName,
    /// What an upstream release tag starts with, followed by a digit (`v`: `v6.18`, `v6.18-rc3`).
    #[serde(default = "default_release_prefix")]
    pub release_prefix: String,
    /// The names branches and tags may have; with none, names are not checked.
    pub scheme: Option<Scheme>,
    /// The branches lists merge: each on the newest release, or on what a `BASE` placeholder says, and equal to
    /// `origin`'s. With none, neither is checked.
    #[serde(default)]
    pub topics: Vec<Pattern>,
    /// The name of the tags assemble makes here: a tag of that shape without a recipe was made by hand.
    pub assembled: Option<Pattern>,
}

fn default_release_prefix() -> String {
    "v".to_string()
}

/// The names a repository's branches and tags may have, beside its release tags.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheme {
    /// The branch names.
    #[serde(default)]
    pub branches: Vec<Pattern>,
    /// The tag names, beside the release tags.
    #[serde(default)]
    pub tags: Vec<Pattern>,
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
    /// Whether it has no change beyond its commit; `None` when it was not read or could not be.
    pub clean: Option<bool>,
}

/// What the integration checks read from a clone; the real reader drives git (rhizoid-cli).
pub trait IntegrationReads {
    /// The refs under `namespace` (`refs/heads/`), without it, each with the commit it names (a tag peeled).
    fn refs(&self, repo: &RepoName, namespace: &str) -> Result<Vec<(String, String)>, RepoError>;
    /// Every annotated tag's name and message, read at once.
    fn tag_messages(&self, repo: &RepoName) -> Result<Vec<(String, String)>, RepoError>;
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
    /// Whether the trees of `a` and `b` are the same.
    fn tree_equal(&self, repo: &RepoName, a: &str, b: &str) -> Result<bool, RepoError>;
    /// The clone's worktrees.
    fn worktrees(&self, repo: &RepoName) -> Result<Vec<Worktree>, RepoError>;
}

/// Every integration edge's findings, in the graph's order.
pub fn integration_status(
    edges: &[Integration],
    git: &dyn IntegrationReads,
) -> Result<Vec<Finding>, RepoError> {
    let mut out = Vec::new();
    for edge in edges {
        let clone = Clone::read(edge, git)?;
        clone.scheme(&mut out);
        clone.topics(&mut out)?;
        clone.against_origin(&mut out)?;
        clone.assembled_tags(&mut out);
        clone.worktrees(&mut out)?;
    }
    Ok(out)
}

/// One clone, its refs and tag messages read once.
struct Clone<'a> {
    edge: &'a Integration,
    git: &'a dyn IntegrationReads,
    heads: BTreeMap<String, String>,
    origin: BTreeMap<String, String>,
    tags: BTreeMap<String, String>,
    messages: BTreeMap<String, String>,
}

impl<'a> Clone<'a> {
    fn read(edge: &'a Integration, git: &'a dyn IntegrationReads) -> Result<Self, RepoError> {
        let refs = |ns: &str| -> Result<BTreeMap<String, String>, RepoError> {
            Ok(git.refs(&edge.repo, ns)?.into_iter().collect())
        };
        Ok(Clone {
            edge,
            git,
            heads: refs("refs/heads/")?,
            origin: refs("refs/remotes/origin/")?,
            tags: refs("refs/tags/")?,
            messages: git.tag_messages(&edge.repo)?.into_iter().collect(),
        })
    }

    fn repo(&self) -> &RepoName {
        &self.edge.repo
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
        tag.strip_prefix(self.edge.release_prefix.as_str())
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    }

    /// The newest release tag: the highest version; release candidates (`-rcN`) do not count, a stable point
    /// release (`v6.18.5`) does.
    fn newest_release(&self) -> Option<&str> {
        self.tags
            .keys()
            .filter(|t| self.is_release(t) && !t.contains("-rc"))
            .max_by_key(|t| release_order(t, &self.edge.release_prefix))
            .map(String::as_str)
    }

    fn scheme(&self, out: &mut Vec<Finding>) {
        let Some(scheme) = &self.edge.scheme else { return };
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
                Check::NameScheme,
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
                Check::NameScheme,
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

    /// The local branches lists merge, each with the pattern it matched.
    fn topic_branches(&self) -> impl Iterator<Item = (&String, &String, &Pattern)> {
        self.heads.iter().filter_map(|(b, sha)| {
            self.edge.topics.iter().find(|p| p.matches(b)).map(|p| (b, sha, p))
        })
    }

    fn topics(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let newest = self.newest_release();
        for (branch, sha, pattern) in self.topic_branches() {
            out.push(match pattern.base_of(branch) {
                Some(base) => self.on_named_base(branch, sha, base)?,
                None => self.on_newest(branch, sha, newest)?,
            });
        }
        Ok(())
    }

    fn on_newest(
        &self,
        branch: &str,
        sha: &str,
        newest: Option<&str>,
    ) -> Result<Finding, RepoError> {
        let Some(newest) = newest else {
            return Ok(self.finding(
                Check::TopicBase,
                "upstream",
                Verdict::NotKnownYet,
                format!("{branch}: the clone has no release tag to read it against"),
            ));
        };
        if self.git.is_ancestor(self.repo(), &self.tags[newest], sha)? {
            return Ok(self.finding(
                Check::TopicBase,
                newest,
                Verdict::Current,
                format!("{branch} is on {newest}, the newest release"),
            ));
        }
        Ok(match self.git.nearest_release(self.repo(), sha, &self.edge.release_prefix)? {
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

    fn on_named_base(&self, branch: &str, sha: &str, base: &str) -> Result<Finding, RepoError> {
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

    /// A topic branch whose local commit is not `origin`'s: assemble takes the local one, so it must be said.
    fn against_origin(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        let mut equal = 0;
        for (branch, local, _) in self.topic_branches() {
            let Some(theirs) = self.origin.get(branch) else { continue };
            if theirs == local {
                equal += 1;
                continue;
            }
            let ahead = self.git.count(self.repo(), theirs, local)?;
            let behind = self.git.count(self.repo(), local, theirs)?;
            let source = format!("origin/{branch}");
            out.push(match (ahead, behind) {
                (0, n) => self.finding(
                    Check::BranchOrigin,
                    &source,
                    Verdict::Behind,
                    format!("{branch} is {n} commits behind origin's: assemble would take the stale local one"),
                ),
                (n, 0) => self.finding(
                    Check::BranchOrigin,
                    &source,
                    Verdict::MovedByHand,
                    format!("{branch} has {n} commits origin does not"),
                ),
                (a, b) => self.finding(
                    Check::BranchOrigin,
                    &source,
                    Verdict::MovedByHand,
                    format!("{branch} has diverged from origin's: {a} commits ahead, {b} behind"),
                ),
            });
        }
        if equal > 0 {
            out.push(self.finding(
                Check::BranchOrigin,
                "origin",
                Verdict::Current,
                format!("{equal} topic branches are origin's"),
            ));
        }
        Ok(())
    }

    /// Each assembled recipe's newest tag, read back against the branches now; a tag of the assembled shape
    /// without a recipe.
    fn assembled_tags(&self, out: &mut Vec<Finding>) {
        let mut newest: BTreeMap<RecipeKey, (u64, &String, Recipe)> = BTreeMap::new();
        for (tag, message) in &self.messages {
            let Some(recipe) = recipe_of(message) else { continue };
            let n = recipe
                .get("n")
                .and_then(|n| n.parse().ok())
                .or_else(|| tag.rsplit('/').next().and_then(|n| n.parse().ok()))
                .unwrap_or(0);
            let field = |k: &str| recipe.get(k).cloned().unwrap_or_default();
            let key = (field("name"), field("list"), field("base"));
            if newest.get(&key).map_or(true, |(best, ..)| n > *best) {
                newest.insert(key, (n, tag, recipe));
            }
        }
        for (_, tag, recipe) in newest.values() {
            out.push(self.recipe(tag, recipe));
        }
        if let Some(shape) = &self.edge.assembled {
            for tag in self.tags.keys().filter(|t| shape.matches(t)) {
                if self.messages.get(tag).and_then(|m| recipe_of(m)).is_none() {
                    out.push(self.finding(
                        Check::Recipe,
                        tag,
                        Verdict::MovedByHand,
                        format!(
                            "{tag} has the shape of an assembled tag but no recipe: made by hand"
                        ),
                    ));
                }
            }
        }
    }

    fn recipe(&self, tag: &str, recipe: &BTreeMap<String, String>) -> Finding {
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
            self.finding(
                Check::Recipe,
                tag,
                Verdict::MovedByHand,
                format!("{tag}'s recipe names no entry: not as rhizoid assemble writes one"),
            )
        } else if moved.is_empty() {
            self.finding(
                Check::Recipe,
                tag,
                Verdict::Current,
                format!("{} entries on {base}; every branch is where the tag took it", i - 1),
            )
        } else {
            self.finding(
                Check::Recipe,
                tag,
                Verdict::Behind,
                format!("{} since it was assembled: assemble again", moved.join(", ")),
            )
        }
    }

    /// rhizoid's worktrees: each locked one against its tag; an unlocked one in a `rhizoid` home was left at a
    /// conflict.
    fn worktrees(&self, out: &mut Vec<Finding>) -> Result<(), RepoError> {
        // the first is the main worktree: never one assemble made
        for w in self.git.worktrees(self.repo())?.into_iter().skip(1) {
            let in_home = std::path::Path::new(&w.path)
                .parent()
                .and_then(|p| p.file_name())
                .is_some_and(|n| n == "rhizoid");
            match w.lock_reason.as_deref().and_then(assembled_for) {
                Some(tag) => out.push(self.locked(&w, tag)?),
                None if in_home && w.lock_reason.is_none() => out.push(self.finding(
                    Check::WorktreeTag,
                    "rhizoid assemble",
                    Verdict::Behind,
                    format!("{}: unlocked, left at a conflict - resolve and commit there, assemble again, then git worktree remove it", w.path),
                )),
                None => {}
            }
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

/// What a recipe is of: its name, list and base.
type RecipeKey = (String, String, String);

/// A recipe's lines, by key.
type Recipe = BTreeMap<String, String>;

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
    use crate::Graph;

    #[test]
    fn releases_order_by_number_and_a_release_above_its_candidates() {
        let o = |t: &str| release_order(t, "v");
        assert!(o("v6.18") > o("v6.18-rc7"));
        assert!(o("v6.18-rc7") > o("v6.18-rc3"));
        assert!(o("v6.18-rc1") > o("v6.9"));
        assert!(o("v6.10") > o("v6.9"));
        assert!(o("v6.1.12") > o("v6.1"));
        assert!(o("v6.18") > o("v2.6.39"));
    }

    #[test]
    fn the_example_kernel_graph_reads() {
        let g = Graph::parse(include_str!("../../../examples/kernel.graph.toml")).unwrap();
        assert_eq!(g.integrations.len(), 1);
        let edge = &g.integrations[0];
        assert!(edge.scheme.as_ref().unwrap().branches.iter().any(|p| p.matches("topic/panel")));
        assert!(edge.topics.iter().any(|p| p.base_of("backport/ufs/v6.12") == Some("v6.12")));
        assert!(edge.assembled.as_ref().unwrap().matches("int/walleye/ufs-off/v6.18/2"));
    }

    #[test]
    fn an_edge_names_only_its_repository_and_checks_only_what_it_is_given() {
        let g = Graph::parse("[[integration]]\nrepo = \"a/b\"\n").unwrap();
        let edge = &g.integrations[0];
        assert!(edge.scheme.is_none() && edge.topics.is_empty() && edge.assembled.is_none());
        assert!(Graph::parse("[[integration]]\nrepo = \"a/b\"\nscheme = { tags = [\"bad//\"] }")
            .unwrap_err()
            .contains("empty part"));
    }
}
