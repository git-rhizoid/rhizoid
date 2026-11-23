//! `assemble`: a named list of branches merged in order onto a base, in a fresh worktree of its own, and tagged
//! by the list file's tag template (`int/{name}/{list}/{base}/{n}` when it gives none) - for any repository.
//!
//! rhizoid owns the list format ([`Lists`]); each user owns its file, and every name in it - the tag template,
//! the branches a list may name - is the file's, not rhizoid's (decision 2026-11-23-rhizoid-stays-general). The
//! merging, the rerere memory and the worktrees are git's own, driven through [`GitPort`] (decision
//! `2026-11-23-standard-tools-drive-cut-or-write`); what is rhizoid's is the list, the order, the refusals,
//! the tag that carries the recipe, and the report a caller's record command takes.

use crate::integration::Worktree;
use crate::pattern::Pattern;
use crate::ports::{BranchSource, GitPort, MergeResult, PortError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// What an assembled worktree's lock reason starts with; the tag's name follows, then `:`.
pub const LOCK_REASON_PREFIX: &str = "rhizoid assemble ";

/// What the message of every tag assemble writes starts with.
pub const TAG_MESSAGE_START: &str = "rhizoid assemble: ";

/// The tag template when a list file gives none.
pub const DEFAULT_TAG: &str = "int/{name}/{list}/{base}/{n}";

/// The tag template when a list file gives neither a template nor a name.
pub const DEFAULT_TAG_WITHOUT_NAME: &str = "int/{list}/{base}/{n}";

/// A list file: an optional name for what it assembles, the tag template, the branches its lists may name, and
/// its named lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lists {
    /// What the file assembles for, as its tags name it (`{name}`); `device` in files written before.
    pub name: Option<String>,
    /// How a tag is named.
    pub tag: TagTemplate,
    /// The branch names a list may name; empty: any branch.
    pub branches: Vec<Pattern>,
    /// The lists by name.
    pub lists: BTreeMap<String, List>,
}

/// One named list: a base and the branches merged onto it, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// What the branches are merged onto: a tag, or a commit.
    pub base: String,
    /// The branches, merged in this order.
    pub entries: Vec<Entry>,
}

/// One branch of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// The branch.
    pub branch: String,
    /// What it is for, in a few words.
    #[serde(rename = "for")]
    pub purpose: String,
    /// Whether it is known to work, and by which test.
    #[serde(flatten)]
    pub state: State,
}

/// Whether an entry is known to work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum State {
    /// Known to work: the test run that showed it, and the commit that run tested.
    KnownGood {
        /// The test run that showed it works.
        test_run: String,
        /// The commit that run tested.
        tested_commit: String,
    },
    /// Being tried: not known to work yet.
    Trying,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLists {
    #[serde(alias = "device")]
    name: Option<String>,
    tag: Option<String>,
    #[serde(default)]
    branches: Vec<Pattern>,
    #[serde(default)]
    lists: BTreeMap<String, FileList>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileList {
    base: String,
    #[serde(default)]
    entries: Vec<FileEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEntry {
    branch: String,
    #[serde(rename = "for")]
    purpose: String,
    state: String,
    test_run: Option<String>,
    tested_commit: Option<String>,
}

impl Lists {
    /// Read a list file (TOML), refusing anything a tag could not carry or the file's branch names do not allow.
    pub fn parse(text: &str) -> Result<Lists, String> {
        let file: FileLists = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(name) = &file.name {
            name_part(name, "name")?;
        }
        let template = file.tag.as_deref().unwrap_or(if file.name.is_some() {
            DEFAULT_TAG
        } else {
            DEFAULT_TAG_WITHOUT_NAME
        });
        let tag = TagTemplate::parse(template, file.name.is_some())?;
        let mut lists = BTreeMap::new();
        for (name, list) in file.lists {
            name_part(&name, "list name")?;
            lists.insert(name.clone(), list_from(&name, list, &file.branches)?);
        }
        if lists.is_empty() {
            return Err("the file names no list ([lists.NAME])".to_string());
        }
        Ok(Lists { name: file.name, tag, branches: file.branches, lists })
    }

    /// The list called `name`, or the names there are.
    pub fn get(&self, name: &str) -> Result<&List, String> {
        self.lists.get(name).ok_or_else(|| {
            let names: Vec<&str> = self.lists.keys().map(String::as_str).collect();
            format!("no list {name}; the file has {}", names.join(", "))
        })
    }
}

fn list_from(name: &str, list: FileList, allowed: &[Pattern]) -> Result<List, String> {
    name_part(&list.base, &format!("lists.{name}.base"))?;
    if list.entries.is_empty() {
        return Err(format!("list {name} has no entries"));
    }
    let mut entries: Vec<Entry> = Vec::new();
    for e in list.entries {
        let at = format!("list {name}, entry {}", e.branch);
        branch_name(&e.branch).map_err(|why| format!("{at}: {why}"))?;
        if !allowed.is_empty() && !allowed.iter().any(|p| p.matches(&e.branch)) {
            let shapes: Vec<&str> = allowed.iter().map(Pattern::as_str).collect();
            return Err(format!("{at}: the file's branches are {}", shapes.join(", ")));
        }
        if entries.iter().any(|seen| seen.branch == e.branch) {
            return Err(format!("{at}: the branch is listed twice"));
        }
        if e.purpose.trim().is_empty() {
            return Err(format!("{at}: `for` is empty - say what it is for"));
        }
        let one_line = [Some(&e.purpose), e.test_run.as_ref(), e.tested_commit.as_ref()];
        if one_line.iter().flatten().any(|s| s.chars().any(char::is_control)) {
            return Err(format!("{at}: for, test_run and tested_commit are one line each"));
        }
        let state = match (e.state.as_str(), e.test_run, e.tested_commit) {
            ("known-good", Some(test_run), Some(tested_commit)) => {
                State::KnownGood { test_run, tested_commit }
            }
            ("known-good", _, _) => {
                return Err(format!("{at}: known-good needs test_run and tested_commit"))
            }
            ("trying", _, _) => State::Trying,
            (other, _, _) => {
                return Err(format!("{at}: state is known-good or trying, not {other}"))
            }
        };
        entries.push(Entry { branch: e.branch, purpose: e.purpose, state });
    }
    Ok(List { base: list.base, entries })
}

/// One part of a ref name, which a report key and a recipe line can carry too: no `/`, no `=`.
fn name_part(s: &str, what: &str) -> Result<(), String> {
    let bad = s.is_empty()
        || s.contains('/')
        || s.starts_with('.')
        || s.starts_with('-')
        || s.ends_with(".lock")
        || s.contains("..")
        || s.contains("@{")
        || s.chars().any(|c| c.is_control() || " ~^:?*[\\={}".contains(c));
    if bad {
        Err(format!("{what} {s:?} cannot be part of a tag name"))
    } else {
        Ok(())
    }
}

/// A branch name a list may carry: `/`-separated parts, each one [`name_part`].
fn branch_name(branch: &str) -> Result<(), String> {
    branch.split('/').try_for_each(|p| name_part(p, "a part")).map_err(|_| {
        format!(
            "{branch:?} is not a branch name a list can carry (no spaces, `=`, `..` or the like)"
        )
    })
}

/// A tag template: `/`-separated text with `{name}`, `{list}`, `{base}` and exactly one `{n}`, the run's number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagTemplate(String);

impl TagTemplate {
    /// Read a template; `{name}` only when the file has a name.
    pub fn parse(text: &str, has_name: bool) -> Result<TagTemplate, String> {
        let mut rest = text;
        let mut numbers = 0;
        while let Some(open) = rest.find('{') {
            let close = rest[open..]
                .find('}')
                .ok_or_else(|| format!("tag {text:?}: a `{{` is not closed"))?;
            match &rest[open + 1..open + close] {
                "n" => numbers += 1,
                "name" if !has_name => {
                    return Err(format!("tag {text:?} names {{name}}, and the file has no name"))
                }
                "name" | "list" | "base" => {}
                other => {
                    return Err(format!("tag {text:?}: {{{other}}} is not name, list, base or n"))
                }
            }
            rest = &rest[open + close + 1..];
        }
        if rest.contains('}') || numbers != 1 {
            return Err(format!("tag {text:?} needs exactly one {{n}}"));
        }
        if text.contains("}{n}") || text.contains("{n}{") {
            return Err(format!(
                "tag {text:?}: {{n}} must not touch another place - put text between them"
            ));
        }
        let sample = text
            .replace("{name}", "x")
            .replace("{list}", "x")
            .replace("{base}", "x")
            .replace("{n}", "1");
        tag_name(&sample)
            .map_err(|_| format!("tag {text:?}: its text is not what a tag name can carry"))?;
        Ok(TagTemplate(text.to_string()))
    }

    /// The tag's text before and after its number, for this list.
    pub fn around_n(&self, name: Option<&str>, list: &str, base: &str) -> (String, String) {
        let text = self
            .0
            .replace("{name}", name.unwrap_or(""))
            .replace("{list}", list)
            .replace("{base}", base);
        let (before, after) = text.split_once("{n}").expect("a parsed template has one {n}");
        (before.to_string(), after.to_string())
    }
}

/// A whole tag name git accepts: each `/`-separated part one ref part.
fn tag_name(tag: &str) -> Result<(), String> {
    tag.split('/')
        .try_for_each(|p| name_part(p, "a part"))
        .map_err(|_| format!("{tag:?} is not a tag name git can carry"))
}

/// The next number among the tags named `before` + digits + `after` (1 when there are none).
pub fn next_number(tags: &[String], before: &str, after: &str) -> Result<u32, String> {
    let highest = tags
        .iter()
        .filter_map(|t| t.strip_prefix(before)?.strip_suffix(after))
        .filter(|n| !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|n| n.parse::<u32>().ok())
        .max();
    match highest {
        None => Ok(1),
        Some(n) => n
            .checked_add(1)
            .ok_or_else(|| format!("{before}{n}{after} is the last number a tag can have")),
    }
}

/// The earlier entries that changed any of the conflicted paths, in list order.
pub fn conflicting_with(conflicted: &[String], earlier: &[(String, Vec<String>)]) -> Vec<String> {
    earlier
        .iter()
        .filter(|(_, paths)| paths.iter().any(|p| conflicted.contains(p)))
        .map(|(branch, _)| branch.clone())
        .collect()
}

/// `wanted` when it is a folder inside `home` (both absolute, no `.` or `..`), else why not.
pub fn worktree_in_home(home: &Path, wanted: &Path) -> Result<PathBuf, String> {
    let plain = |p: &Path| {
        p.is_absolute()
            && p.components().all(|c| matches!(c, Component::Normal(_) | Component::RootDir))
    };
    if !plain(home) {
        return Err(format!("the worktree home {} is not an absolute plain path", home.display()));
    }
    if !plain(wanted) || !wanted.starts_with(home) || wanted == home {
        return Err(format!(
            "refused: the worktree {} is outside rhizoid's home {} (ideal-agent's worktree home, where prune sees it)",
            wanted.display(),
            home.display()
        ));
    }
    Ok(wanted.to_path_buf())
}

/// The first free worktree folder in `home` for a tag: the tag with `-` for `/`, then `-2`, `-3`, ...
pub fn default_worktree(home: &Path, tag: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let name = tag.replace('/', "-");
    let first = home.join(&name);
    if !taken(&first) {
        return first;
    }
    (2u32..)
        .map(|k| home.join(format!("{name}-{k}")))
        .find(|p| !taken(p))
        .expect("an unbounded range has a free name")
}

/// What assemble is asked: one list of a list file, where its worktree goes, and what to compare it with.
pub struct Ask<'a> {
    /// The file's name, if it has one (`{name}`).
    pub name: Option<&'a str>,
    /// How the tag is named.
    pub tag: &'a TagTemplate,
    /// The list's name.
    pub list_name: &'a str,
    /// The list.
    pub list: &'a List,
    /// rhizoid's worktree home for this clone: `AGENT_WORKTREES/REPO/rhizoid`.
    pub home: &'a Path,
    /// The worktree folder asked for, if any (it must be inside `home`); otherwise the first free default.
    pub worktree: Option<&'a Path>,
    /// Fetch every remote, pruning, first.
    pub fetch: bool,
    /// A commit whose tree the result must equal (one built and tested by hand).
    pub reproduces: Option<&'a str>,
}
/// One entry as it was merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Merged {
    /// The entry as listed; a known-good entry's tested commit as a full sha.
    #[serde(flatten)]
    pub entry: Entry,
    /// The commit its branch was at.
    pub sha: String,
    /// Where the branch was found: the clone's own, or `origin`'s.
    pub source: BranchSource,
    /// How it went in.
    pub merged: How,
}

/// How an entry went in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum How {
    /// A merge commit, without a conflict.
    Merged,
    /// A merge commit; its conflict was resolved from rerere's recorded resolution.
    RerereResolved,
    /// Nothing: the base or an earlier entry already held it.
    AlreadyIn,
}

impl How {
    fn word(self) -> &'static str {
        match self {
            How::Merged => "merged",
            How::RerereResolved => "rerere-resolved",
            How::AlreadyIn => "already-in",
        }
    }

    fn from_word(w: &str) -> How {
        match w {
            "rerere-resolved" => How::RerereResolved,
            "already-in" => How::AlreadyIn,
            _ => How::Merged,
        }
    }
}

fn source_word(s: BranchSource) -> &'static str {
    match s {
        BranchSource::Local => "local",
        BranchSource::Origin => "origin",
    }
}

/// Where a run stopped at a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    /// The entry whose merge stopped.
    pub entry: String,
    /// The earlier entries that changed the same paths (empty: the base itself, or nothing earlier did).
    pub with: Vec<String>,
    /// The paths left in conflict.
    pub paths: Vec<String>,
}

/// The comparison `--reproduces` asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reproduces {
    /// The commit compared with.
    pub commit: String,
    /// The tag's tree equals the commit's tree.
    pub equal: bool,
    /// The paths that differ.
    pub paths: Vec<String>,
}

/// What a run did: the recipe, and the tag, or the conflict it stopped at. Printed as `key=value` lines or as
/// JSON; a field added later is a new key, so readers keep working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The list file's name, if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The list's name.
    pub list: String,
    /// The base as listed.
    pub base: String,
    /// The commit the base named.
    pub base_sha: String,
    /// The entries merged, in order (up to the one that stopped, when one did).
    pub entries: Vec<Merged>,
    /// The worktree: locked once tagged, unlocked when a conflict stopped the run; none when an unchanged list
    /// found its tag and that tag has no worktree any more.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// The tag's number among the tags of this list and base.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// The tag made, or the existing tag of the same recipe (none when a conflict stopped the run).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The tag object's sha.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_sha: Option<String>,
    /// The commit the tag points at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The base and every entry equal an existing tag's recipe: that tag is the result, none was made.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
    /// The conflict the run stopped at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<Conflict>,
    /// The comparison `--reproduces` asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduces: Option<Reproduces>,
}

impl Report {
    /// The run did what was asked: tagged, and equal to the commit it had to reproduce, if any.
    pub fn ok(&self) -> bool {
        self.conflict.is_none() && self.reproduces.as_ref().map_or(true, |r| r.equal)
    }

    /// The recipe: name, list, base and every entry - what the tag's message carries.
    pub fn recipe(&self) -> Vec<(String, String)> {
        let mut kv = Vec::new();
        if let Some(name) = &self.name {
            kv.push(("name".to_string(), name.clone()));
        }
        kv.push(("list".to_string(), self.list.clone()));
        kv.push(("base".to_string(), self.base.clone()));
        kv.push(("base_sha".to_string(), self.base_sha.clone()));
        if let Some(n) = self.n {
            kv.push(("n".to_string(), n.to_string()));
        }
        for (i, m) in self.entries.iter().enumerate() {
            let k = |f: &str| format!("entry.{}.{f}", i + 1);
            kv.push((k("branch"), m.entry.branch.clone()));
            kv.push((k("sha"), m.sha.clone()));
            kv.push((k("source"), source_word(m.source).to_string()));
            kv.push((k("for"), m.entry.purpose.clone()));
            match &m.entry.state {
                State::KnownGood { test_run, tested_commit } => {
                    kv.push((k("state"), "known-good".to_string()));
                    kv.push((k("test_run"), test_run.clone()));
                    kv.push((k("tested_commit"), tested_commit.clone()));
                }
                State::Trying => kv.push((k("state"), "trying".to_string())),
            }
            kv.push((k("merged"), m.merged.word().to_string()));
        }
        kv
    }

    /// Every field as `key=value` pairs: the recipe, the record lines (one per entry, its branch with `.` for
    /// `/` = its sha; `tested=`), then the outcome.
    pub fn key_values(&self) -> Vec<(String, String)> {
        let mut kv = self.recipe();
        let mut put = |k: &str, v: &str| kv.push((k.to_string(), v.to_string()));
        for m in &self.entries {
            put(&m.entry.branch.replace('/', "."), &m.sha);
        }
        let mut tested: Vec<&str> = Vec::new();
        for m in &self.entries {
            if let State::KnownGood { tested_commit, .. } = &m.entry.state {
                if !tested.contains(&tested_commit.as_str()) {
                    tested.push(tested_commit);
                }
            }
        }
        if !tested.is_empty() {
            put("tested", &tested.join(" "));
        }
        if let Some(w) = &self.worktree {
            put("worktree", w);
        }
        if let (Some(tag), Some(sha), Some(commit)) = (&self.tag, &self.tag_sha, &self.commit) {
            put("tag", tag);
            put("tag_sha", sha);
            put("commit", commit);
        }
        if self.unchanged {
            put("unchanged", "yes");
        }
        if let Some(c) = &self.conflict {
            put("conflict.entry", &c.entry);
            put("conflict.with", &c.with.join(" "));
            for (i, p) in c.paths.iter().enumerate() {
                put(&format!("conflict.path.{}", i + 1), p);
            }
        }
        if let Some(r) = &self.reproduces {
            put("reproduces.commit", &r.commit);
            put("reproduces", if r.equal { "equal" } else { "differs" });
            for (i, p) in r.paths.iter().enumerate() {
                put(&format!("reproduces.path.{}", i + 1), p);
            }
        }
        kv
    }
}

/// The recipe lines that say what went in: everything but how it went in, where a branch was found and the
/// number - two runs with these equal made the same tree.
fn what_went_in(kv: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    kv.into_iter()
        .filter(|(k, _)| k != "n" && !k.ends_with(".merged") && !k.ends_with(".source"))
        .collect()
}

/// Why assemble did not run, or stopped before a report: nothing it says was made, unless it says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembleError(pub String);

impl std::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<PortError> for AssembleError {
    fn from(e: PortError) -> Self {
        AssembleError(e.0)
    }
}

/// Assemble one list: everything is read and checked first; a list whose base and entries are an existing
/// tag's recipe is that tag; otherwise each branch is merged in order into a fresh worktree, stopping at a
/// conflict rerere cannot resolve (the worktree left unlocked, for a hand to resolve), then the tag carries the
/// recipe and the worktree is locked until `done`.
pub fn assemble(git: &impl GitPort, ask: &Ask) -> Result<Report, AssembleError> {
    if ask.fetch {
        git.fetch_prune()?;
    }
    let base_sha = base_commit(git, &ask.list.base)?;
    let entries = entries_with_tested_shas(git, &ask.list.entries)?;
    let branches = entry_commits(git, &entries)?;
    let reproduces = ask.reproduces.map(|c| commit_or_refuse(git, c)).transpose()?;
    let (before, after) = ask.tag.around_n(ask.name, ask.list_name, &ask.list.base);
    let tags = git.tags(&before)?;
    let mut report = Report {
        name: ask.name.map(str::to_string),
        list: ask.list_name.to_string(),
        base: ask.list.base.clone(),
        base_sha: base_sha.clone(),
        entries: entries
            .iter()
            .zip(&branches)
            .map(|(e, (sha, source))| Merged {
                entry: e.clone(),
                sha: sha.clone(),
                source: *source,
                merged: How::Merged,
            })
            .collect(),
        worktree: None,
        n: None,
        tag: None,
        tag_sha: None,
        commit: None,
        unchanged: false,
        conflict: None,
        reproduces: None,
    };

    if let Some((tag, tag_sha, recipe)) = same_recipe(git, &tags, &report)? {
        if ask.worktree.is_some() {
            return Err(AssembleError(format!(
                "the list is unchanged since {tag}, which is the result: --worktree asks for a fresh one (change the list, or drop --worktree)"
            )));
        }
        for (i, m) in report.entries.iter_mut().enumerate() {
            let how = recipe.get(&format!("entry.{}.merged", i + 1)).map_or("", String::as_str);
            m.merged = How::from_word(how);
        }
        report.n = recipe.get("n").and_then(|n| n.parse().ok());
        report.worktree = worktree_of(git, &tag)?.map(|w| w.path);
        report.commit = git.commit(&tag)?;
        report.tag_sha = Some(tag_sha);
        report.tag = Some(tag.clone());
        report.unchanged = true;
        compare(git, &mut report, &tag, reproduces)?;
        return Ok(report);
    }

    let n = next_number(&tags, &before, &after).map_err(AssembleError)?;
    let tag = format!("{before}{n}{after}");
    tag_name(&tag).map_err(AssembleError)?;
    let worktree = worktree_for(ask, &tag)?;
    git.share_rerere()?;
    git.add_worktree(&worktree, &base_sha)?;
    report.worktree = Some(worktree.display().to_string());
    report.n = Some(n);
    let planned = std::mem::take(&mut report.entries);
    for m in planned {
        let message =
            format!("rhizoid assemble {tag}: merge {} ({})", m.entry.branch, m.entry.purpose);
        let merged = match git.merge(&worktree, &m.sha, &message)? {
            MergeResult::Merged { by_rerere: true } => How::RerereResolved,
            MergeResult::Merged { by_rerere: false } => How::Merged,
            MergeResult::AlreadyContained => How::AlreadyIn,
            MergeResult::Conflict { paths } => {
                report.conflict = Some(conflict(git, &base_sha, &report.entries, &m.entry, paths)?);
                return Ok(report);
            }
        };
        report.entries.push(Merged { merged, ..m });
    }

    let commit = git.head(&worktree)?;
    report.tag_sha = Some(git.tag(&tag, &commit, &tag_message(&report))?);
    report.tag = Some(tag.clone());
    report.commit = Some(commit);
    git.lock_worktree(
        &worktree,
        &format!("{LOCK_REASON_PREFIX}{tag}: wanted until rhizoid assemble --done {tag}"),
    )?;
    compare(git, &mut report, &tag, reproduces)?;
    Ok(report)
}

/// End a tag's worktree: remove it, locked as it is; refused when it has changes, unless `force`. Its folder.
pub fn done(git: &impl GitPort, tag: &str, force: bool) -> Result<String, AssembleError> {
    let worktree = worktree_of(git, tag)?.ok_or_else(|| {
        AssembleError(format!("no worktree of {tag} (rhizoid assemble locks each one it tags)"))
    })?;
    let at_tag = worktree.head.is_some() && worktree.head == git.commit(tag)?;
    if !at_tag && !force {
        return Err(AssembleError(format!(
            "refused: {} has commits beyond {tag} - --force removes it anyway, and them",
            worktree.path
        )));
    }
    match worktree.clean {
        Some(true) => {}
        _ if force => {}
        Some(false) => {
            return Err(AssembleError(format!(
                "refused: {} has changes beyond {tag} - --force removes it anyway",
                worktree.path
            )))
        }
        None => {
            return Err(AssembleError(format!(
                "refused: {} could not be read (gone?) - --force removes what git knows of it",
                worktree.path
            )))
        }
    }
    git.remove_worktree(Path::new(&worktree.path))?;
    Ok(worktree.path)
}

/// The worktree assemble locked for `tag`, if one is left; of several, the one at the tag's commit.
fn worktree_of(git: &impl GitPort, tag: &str) -> Result<Option<Worktree>, AssembleError> {
    let mut found: Vec<Worktree> = git
        .worktrees()?
        .into_iter()
        .filter(|w| w.lock_reason.as_deref().and_then(assembled_for) == Some(tag))
        .collect();
    if found.len() > 1 {
        let at = git.commit(tag)?;
        found.retain(|w| w.head.is_some() && w.head == at);
        if found.len() != 1 {
            return Err(AssembleError(format!(
                "several worktrees are locked for {tag} and not one of them is at it: end them with git worktree remove"
            )));
        }
    }
    Ok(found.pop())
}

/// The tag a lock reason assemble wrote names (`rhizoid assemble TAG: ...`).
pub fn assembled_for(reason: &str) -> Option<&str> {
    reason.strip_prefix(LOCK_REASON_PREFIX)?.split_once(':').map(|(tag, _)| tag)
}

/// The recipe lines (`key=value`) of a tag's message, when assemble wrote it.
pub fn recipe_of(message: &str) -> Option<BTreeMap<String, String>> {
    if !message.starts_with(TAG_MESSAGE_START) {
        return None;
    }
    let mut recipe: BTreeMap<String, String> = message
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    // a recipe of v0.1.1 named its name `device`
    if let Some(device) = recipe.remove("device") {
        recipe.entry("name".to_string()).or_insert(device);
    }
    Some(recipe)
}

/// The newest existing tag whose recipe says the same went in (base, every entry with its sha, what it is for
/// and its state): its name, its object's sha and its recipe.
#[allow(clippy::type_complexity)]
fn same_recipe(
    git: &impl GitPort,
    tags: &[String],
    report: &Report,
) -> Result<Option<(String, String, BTreeMap<String, String>)>, AssembleError> {
    let wanted = what_went_in(report.recipe());
    let mut found: Option<(u64, String, String, BTreeMap<String, String>)> = None;
    for tag in tags {
        let (object, message) = git.tag_object(tag)?;
        let Some(recipe) = recipe_of(&message) else { continue };
        if what_went_in(recipe.clone()) != wanted {
            continue;
        }
        let n = recipe.get("n").and_then(|n| n.parse().ok()).unwrap_or(0);
        if found.as_ref().map_or(true, |(best, ..)| n >= *best) {
            found = Some((n, tag.clone(), object, recipe));
        }
    }
    Ok(found.map(|(_, tag, object, recipe)| (tag, object, recipe)))
}

fn compare(
    git: &impl GitPort,
    report: &mut Report,
    tag: &str,
    reproduces: Option<String>,
) -> Result<(), AssembleError> {
    if let Some(other) = reproduces {
        let paths = git.tree_diff(tag, &other)?;
        report.reproduces = Some(Reproduces { commit: other, equal: paths.is_empty(), paths });
    }
    Ok(())
}

fn base_commit(git: &impl GitPort, base: &str) -> Result<String, AssembleError> {
    git.commit(base)?.ok_or_else(|| {
        AssembleError(format!("the base {base} is not in the clone (fetch it, or --fetch)"))
    })
}

fn commit_or_refuse(git: &impl GitPort, rev: &str) -> Result<String, AssembleError> {
    git.commit(rev)?
        .ok_or_else(|| AssembleError(format!("--reproduces {rev}: no such commit in the clone")))
}

/// The entries, each known-good one's tested commit resolved to a full sha: it must be a commit's sha (or a
/// prefix naming one commit), read as a sha and never as a name - HEAD, a branch or a tag move.
fn entries_with_tested_shas(
    git: &impl GitPort,
    entries: &[Entry],
) -> Result<Vec<Entry>, AssembleError> {
    entries
        .iter()
        .map(|e| match &e.state {
            State::Trying => Ok(e.clone()),
            State::KnownGood { test_run, tested_commit } => {
                let full = git.commit_by_sha(tested_commit)?.ok_or_else(|| {
                    AssembleError(format!(
                        "{}: tested_commit {tested_commit} is not a commit's sha in the clone (a name that moves, like HEAD or a branch, cannot say what was tested)",
                        e.branch
                    ))
                })?;
                Ok(Entry {
                    state: State::KnownGood { test_run: test_run.clone(), tested_commit: full },
                    ..e.clone()
                })
            }
        })
        .collect()
}

fn entry_commits(
    git: &impl GitPort,
    entries: &[Entry],
) -> Result<Vec<(String, BranchSource)>, AssembleError> {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for e in entries {
        match git.branch(&e.branch)? {
            Some(b) => found.push(b),
            None => missing.push(e.branch.as_str()),
        }
    }
    if missing.is_empty() {
        Ok(found)
    } else {
        Err(AssembleError(format!("no branch {} (locally or on origin)", missing.join(", "))))
    }
}

fn worktree_for(ask: &Ask, tag: &str) -> Result<PathBuf, AssembleError> {
    let path = match ask.worktree {
        Some(w) => worktree_in_home(ask.home, w).map_err(AssembleError)?,
        None => default_worktree(ask.home, tag, |p| p.exists()),
    };
    if path.exists() {
        return Err(AssembleError(format!(
            "refused: {} exists - a worktree is made fresh each time",
            path.display()
        )));
    }
    Ok(path)
}

fn conflict(
    git: &impl GitPort,
    base_sha: &str,
    merged: &[Merged],
    entry: &Entry,
    paths: Vec<String>,
) -> Result<Conflict, AssembleError> {
    let mut earlier = Vec::new();
    for m in merged {
        earlier.push((m.entry.branch.clone(), git.changed_paths(base_sha, &m.sha)?));
    }
    Ok(Conflict { entry: entry.branch.clone(), with: conflicting_with(&paths, &earlier), paths })
}

fn tag_message(report: &Report) -> String {
    let what = match &report.name {
        Some(name) => format!("{name}/{}", report.list),
        None => report.list.clone(),
    };
    let mut m = format!("{TAG_MESSAGE_START}{what} onto {}\n\n", report.base);
    for (k, v) in report.recipe() {
        m.push_str(&format!("{k}={v}\n"));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(lists: &str) -> Result<Lists, String> {
        Lists::parse(&format!("name = \"walleye\"\n{lists}"))
    }

    const ONE: &str = "[lists.ufs-off]\nbase = \"v6.18\"\n[[lists.ufs-off.entries]]\n";

    #[test]
    fn a_list_reads_its_base_and_entries_in_order() {
        let l = parse(&format!(
            "{ONE}branch = \"topic/panel\"\nfor = \"display\"\nstate = \"known-good\"\ntest_run = \"r1\"\ntested_commit = \"abc\"\n\
             [[lists.ufs-off.entries]]\nbranch = \"backport/ufs/v6.12\"\nfor = \"ufs\"\nstate = \"trying\"\n"
        ))
        .unwrap();
        let list = l.get("ufs-off").unwrap();
        assert_eq!(list.base, "v6.18");
        assert_eq!(list.entries[0].branch, "topic/panel");
        assert_eq!(
            list.entries[0].state,
            State::KnownGood { test_run: "r1".into(), tested_commit: "abc".into() }
        );
        assert_eq!(list.entries[1].state, State::Trying);
        assert!(l.get("ufs-on").unwrap_err().contains("the file has ufs-off"));
    }

    #[test]
    fn any_branch_unless_the_file_names_the_shapes_its_lists_may_carry() {
        let entry = |head: &str, branch: &str| {
            Lists::parse(&format!(
                "{head}\n[lists.a]\nbase = \"v1\"\n[[lists.a.entries]]\nbranch = \"{branch}\"\nfor = \"x\"\nstate = \"trying\"\n"
            ))
        };
        assert!(entry("", "feature/login").is_ok(), "no shapes given: any branch");
        assert!(entry("", "wip/x").is_ok());
        assert!(entry("", "has space").is_err());
        assert!(entry("", "a=b").is_err(), "a report key cannot carry =");
        assert!(Lists::parse("[lists.\"{n}\"]\nbase = \"v1\"\n[[lists.\"{n}\".entries]]\nbranch = \"x\"\nfor = \"x\"\nstate = \"trying\"\n").is_err(), "braces in a name");
        let shapes = "branches = [\"topic/ITEM\", \"backport/ITEM/BASE\"]";
        assert!(entry(shapes, "topic/a").is_ok());
        assert!(entry(shapes, "wip/x").unwrap_err().contains("topic/ITEM, backport/ITEM/BASE"));
    }

    #[test]
    fn what_a_tag_or_a_state_cannot_carry_is_refused() {
        let entry = |branch: &str, rest: &str| {
            parse(&format!("{ONE}branch = \"{branch}\"\nfor = \"x\"\n{rest}"))
        };
        assert!(entry("topic/a", "state = \"good\"").unwrap_err().contains("known-good or trying"));
        assert!(entry("topic/a", "state = \"known-good\"").unwrap_err().contains("test_run"));
        assert!(entry("topic/a", "state = \"trying\"\nextra = 1").is_err(), "unknown keys");
        assert!(parse("[lists.a]\nbase = \"v1/x\"\n[[lists.a.entries]]\nbranch = \"topic/a\"\nfor = \"x\"\nstate = \"trying\"\n")
            .unwrap_err()
            .contains("cannot be part of a tag name"));
        assert!(parse("[lists.a]\nbase = \"v1\"\n").unwrap_err().contains("no entries"));
        assert!(parse("").unwrap_err().contains("no list"));
    }

    #[test]
    fn the_name_is_optional_and_device_is_read_as_name() {
        let list = "[lists.a]\nbase = \"v1\"\n[[lists.a.entries]]\nbranch = \"x\"\nfor = \"x\"\nstate = \"trying\"\n";
        let named = Lists::parse(&format!("device = \"d\"\n{list}")).unwrap();
        assert_eq!(named.name.as_deref(), Some("d"));
        assert_eq!(
            named.tag.around_n(Some("d"), "a", "v1"),
            ("int/d/a/v1/".to_string(), String::new())
        );
        let plain = Lists::parse(list).unwrap();
        assert_eq!(plain.name, None);
        assert_eq!(plain.tag.around_n(None, "a", "v1"), ("int/a/v1/".to_string(), String::new()));
    }

    #[test]
    fn a_tag_template_has_one_number_and_known_places() {
        assert!(TagTemplate::parse("build/{list}-{n}", false).is_ok());
        let t = TagTemplate::parse("rc/{list}/{base}.{n}-test", false).unwrap();
        assert_eq!(t.around_n(None, "l", "b"), ("rc/l/b.".to_string(), "-test".to_string()));
        assert!(TagTemplate::parse("x/{list}", false).unwrap_err().contains("exactly one {n}"));
        assert!(TagTemplate::parse("x/{n}/{n}", false).is_err());
        assert!(TagTemplate::parse("x/{name}/{n}", false).unwrap_err().contains("no name"));
        assert!(TagTemplate::parse("x/{device}/{n}", true)
            .unwrap_err()
            .contains("not name, list, base or n"));
        assert!(TagTemplate::parse("x/{n", true).is_err());
        assert!(TagTemplate::parse("x/{list}{n}", false).unwrap_err().contains("must not touch"));
        assert!(TagTemplate::parse("-x/{list}/{n}", false)
            .unwrap_err()
            .contains("not what a tag name"));
        assert!(TagTemplate::parse("x*/{list}/{n}", false).is_err());
    }

    #[test]
    fn the_next_number_follows_the_highest_tag_of_the_same_shape() {
        let tags = |t: &[&str]| t.iter().map(|t| t.to_string()).collect::<Vec<String>>();
        assert_eq!(next_number(&[], "int/dev/two/v1/", ""), Ok(1));
        let some = tags(&[
            "int/dev/two/v1/1",
            "int/dev/two/v1/7",
            "int/dev/two/v1/x",
            "int/dev/two/v1/+9",
            "int/dev/two/v2/9",
        ]);
        assert_eq!(next_number(&some, "int/dev/two/v1/", ""), Ok(8));
        assert_eq!(next_number(&tags(&["rc/l.3-test", "rc/l.4"]), "rc/l.", "-test"), Ok(4));
        assert!(next_number(&tags(&["int/dev/two/v1/4294967295"]), "int/dev/two/v1/", "").is_err());
    }

    #[test]
    fn a_conflict_names_the_earlier_entries_that_touched_its_paths() {
        let earlier = vec![
            ("topic/a".to_string(), vec!["a.c".to_string()]),
            ("topic/b".to_string(), vec!["b.c".to_string()]),
        ];
        assert_eq!(conflicting_with(&["a.c".to_string()], &earlier), vec!["topic/a"]);
        assert!(conflicting_with(&["z.c".to_string()], &earlier).is_empty());
    }

    #[test]
    fn a_worktree_is_inside_the_home_or_refused() {
        let home = Path::new("/w/linux/rhizoid");
        assert!(worktree_in_home(home, Path::new("/w/linux/rhizoid/x")).is_ok());
        assert!(worktree_in_home(home, Path::new("/w/linux/rhizoid")).is_err());
        assert!(worktree_in_home(home, Path::new("/w/linux/rhizoid/../x")).is_err());
        assert!(worktree_in_home(home, Path::new("/elsewhere")).is_err());
        assert!(worktree_in_home(home, Path::new("rel")).is_err());
    }

    #[test]
    fn the_example_kernel_list_file_reads() {
        let l = Lists::parse(include_str!("../../../examples/kernel-topics.toml")).unwrap();
        assert_eq!(l.name.as_deref(), Some("walleye"));
        assert_eq!(l.lists.len(), 2);
        assert_eq!(l.branches.len(), 2);
    }

    #[test]
    fn the_default_worktree_is_the_tag_and_then_the_first_free_name() {
        let home = Path::new("/w");
        let taken = |p: &Path| p == Path::new("/w/int-l-b-1") || p == Path::new("/w/int-l-b-1-2");
        assert_eq!(default_worktree(home, "int/l/b/1", taken), PathBuf::from("/w/int-l-b-1-3"));
        assert_eq!(default_worktree(home, "int/l/b/2", taken), PathBuf::from("/w/int-l-b-2"));
    }

    #[test]
    fn a_recipe_written_with_device_reads_as_name() {
        let r = recipe_of("rhizoid assemble: d/l onto b\n\ndevice=d\nlist=l\n").unwrap();
        assert_eq!(r.get("name").map(String::as_str), Some("d"));
        assert!(!r.contains_key("device"));
    }

    #[test]
    fn two_recipes_say_the_same_went_in_whatever_the_number_and_how() {
        let a = [
            ("n", "1"),
            ("entry.1.merged", "merged"),
            ("entry.1.sha", "x"),
            ("entry.1.source", "local"),
        ];
        let b = [
            ("n", "2"),
            ("entry.1.merged", "already-in"),
            ("entry.1.sha", "x"),
            ("entry.1.source", "origin"),
        ];
        let kv = |p: &[(&str, &str)]| {
            p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>()
        };
        assert_eq!(what_went_in(kv(&a)), what_went_in(kv(&b)));
        assert_ne!(what_went_in(kv(&a)), what_went_in(kv(&[("entry.1.sha", "y")])));
    }
}
