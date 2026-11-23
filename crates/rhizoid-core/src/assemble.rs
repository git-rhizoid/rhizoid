//! `assemble`: a named list of branches merged in order onto a fresh base, in a worktree of its own, and
//! tagged `int/DEVICE/LIST/BASE/N` - the integration a test kernel is built from.
//!
//! rhizoid owns the list format ([`Lists`]); each lab owns its file. The merging, the rerere memory and the
//! worktrees are git's own, driven through [`GitPort`] (decision
//! `2026-11-23-standard-tools-drive-cut-or-write`); what is rhizoid's is the list, the order, the refusals,
//! the tag that carries the recipe, and the report a caller's record command takes.

use crate::kernel::Worktree;
use crate::ports::{BranchSource, GitPort, MergeResult, PortError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// What an assembled worktree's lock reason starts with; the tag's name follows, then `:`.
pub const LOCK_REASON_PREFIX: &str = "rhizoid assemble ";

/// What the message of every tag assemble writes starts with.
pub const TAG_MESSAGE_START: &str = "rhizoid assemble: ";

/// A lab's list file: the device it assembles for, and its named lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lists {
    /// The device the tags are named for (`int/DEVICE/...`).
    pub device: String,
    /// The lists by name (walleye: `ufs-off` and `ufs-on`, never one tag for both).
    pub lists: BTreeMap<String, List>,
}

/// One named list: a base and the branches merged onto it, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// What the branches are merged onto: a tag, or a fork's commit.
    pub base: String,
    /// The branches, merged in this order.
    pub entries: Vec<Entry>,
}

/// One branch of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// `topic/ITEM` or `backport/ITEM/BASE`.
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
    device: String,
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
    /// Read a list file (TOML), refusing anything a tag or a branch scheme could not carry.
    pub fn parse(text: &str) -> Result<Lists, String> {
        let file: FileLists = toml::from_str(text).map_err(|e| e.to_string())?;
        name_part(&file.device, "device")?;
        let mut lists = BTreeMap::new();
        for (name, list) in file.lists {
            name_part(&name, "list name")?;
            lists.insert(name.clone(), list_from(&name, list)?);
        }
        if lists.is_empty() {
            return Err("the file names no list ([lists.NAME])".to_string());
        }
        Ok(Lists { device: file.device, lists })
    }

    /// The list called `name`, or the names there are.
    pub fn get(&self, name: &str) -> Result<&List, String> {
        self.lists.get(name).ok_or_else(|| {
            let names: Vec<&str> = self.lists.keys().map(String::as_str).collect();
            format!("no list {name}; the file has {}", names.join(", "))
        })
    }
}

fn list_from(name: &str, list: FileList) -> Result<List, String> {
    name_part(&list.base, &format!("lists.{name}.base"))?;
    if list.entries.is_empty() {
        return Err(format!("list {name} has no entries"));
    }
    let mut entries: Vec<Entry> = Vec::new();
    for e in list.entries {
        let at = format!("list {name}, entry {}", e.branch);
        branch_in_scheme(&e.branch).map_err(|why| format!("{at}: {why}"))?;
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

/// A part of a tag name (`int/DEVICE/LIST/BASE/N`): one component git accepts in a ref.
fn name_part(s: &str, what: &str) -> Result<(), String> {
    let bad = s.is_empty()
        || s.contains('/')
        || s.starts_with('.')
        || s.starts_with('-')
        || s.ends_with(".lock")
        || s.contains("..")
        || s.contains("@{")
        || s.chars().any(|c| c.is_control() || " ~^:?*[\\".contains(c));
    if bad {
        Err(format!("{what} {s:?} cannot be part of a tag name"))
    } else {
        Ok(())
    }
}

/// `topic/ITEM` or `backport/ITEM/BASE`, each part one ref component.
fn branch_in_scheme(branch: &str) -> Result<(), String> {
    let parts: Vec<&str> = branch.split('/').collect();
    let fits = match parts.as_slice() {
        ["topic", item] => name_part(item, "item").is_ok(),
        ["backport", item, base] => {
            name_part(item, "item").is_ok() && name_part(base, "base").is_ok()
        }
        _ => false,
    };
    if fits {
        Ok(())
    } else {
        Err("a branch is topic/ITEM or backport/ITEM/BASE".to_string())
    }
}

/// The tag name without its number: `int/DEVICE/LIST/BASE/`.
pub fn tag_prefix(device: &str, list: &str, base: &str) -> String {
    format!("int/{device}/{list}/{base}/")
}

/// The next number after the tags already under `prefix` (1 when there are none); only digits count.
pub fn next_number(tags: &[String], prefix: &str) -> Result<u32, String> {
    let highest = tags
        .iter()
        .filter_map(|t| t.strip_prefix(prefix))
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|n| n.parse::<u32>().ok())
        .max();
    match highest {
        None => Ok(1),
        Some(n) => {
            n.checked_add(1).ok_or_else(|| format!("{prefix}{n} is the last number a tag can have"))
        }
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

/// The first free worktree folder in `home` for this tag: `DEVICE-LIST-BASE-N`, then `-2`, `-3`, ...
pub fn default_worktree(
    home: &Path,
    device: &str,
    list: &str,
    base: &str,
    n: u32,
    taken: impl Fn(&Path) -> bool,
) -> PathBuf {
    let name = format!("{device}-{list}-{base}-{n}");
    let first = home.join(&name);
    if !taken(&first) {
        return first;
    }
    (2u32..)
        .map(|k| home.join(format!("{name}-{k}")))
        .find(|p| !taken(p))
        .expect("an unbounded range has a free name")
}

/// What assemble is asked: one list of a lab's file, where its worktree goes, and what to compare it with.
pub struct Ask<'a> {
    /// The device the tag is named for.
    pub device: &'a str,
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
    /// A commit whose tree the result must equal (a kernel that was built and tested by hand).
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
    /// The device.
    pub device: String,
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
    /// The tag made, or the existing tag of the same recipe (none when a conflict stopped the run).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The tag object's sha.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_sha: Option<String>,
    /// The commit the tag points at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The base and every entry's sha equal an existing tag's recipe: that tag is the result, none was made.
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

    /// The recipe: device, list, base and every entry - what the tag's message carries.
    pub fn recipe(&self) -> Vec<(String, String)> {
        let mut kv = vec![
            ("device".to_string(), self.device.clone()),
            ("list".to_string(), self.list.clone()),
            ("base".to_string(), self.base.clone()),
            ("base_sha".to_string(), self.base_sha.clone()),
        ];
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

    /// Every field as `key=value` pairs: the recipe, the record lines (`topic.ITEM=sha`, `tested=`), then the
    /// outcome.
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

/// Assemble one list: everything is read and checked first; a list whose base and branches are an existing
/// tag's is that tag; otherwise each branch is merged in order into a fresh worktree, stopping at a conflict
/// rerere cannot resolve (the worktree left unlocked, for a hand to resolve), then the tag carries the recipe and
/// the worktree is locked until `done`.
pub fn assemble(git: &impl GitPort, ask: &Ask) -> Result<Report, AssembleError> {
    if ask.fetch {
        git.fetch_prune()?;
    }
    let base_sha = base_commit(git, &ask.list.base)?;
    let entries = entries_with_tested_shas(git, &ask.list.entries)?;
    let branches = entry_commits(git, &entries)?;
    let reproduces = ask.reproduces.map(|c| commit_or_refuse(git, c)).transpose()?;
    let prefix = tag_prefix(ask.device, ask.list_name, &ask.list.base);
    let tags = git.tags(&prefix)?;
    let mut report = Report {
        device: ask.device.to_string(),
        list: ask.list_name.to_string(),
        base: ask.list.base.clone(),
        base_sha: base_sha.clone(),
        entries: Vec::new(),
        worktree: None,
        tag: None,
        tag_sha: None,
        commit: None,
        unchanged: false,
        conflict: None,
        reproduces: None,
    };

    if let Some((tag, tag_sha, recipe)) = same_recipe(git, &tags, &base_sha, &entries, &branches)? {
        report.entries = entries
            .iter()
            .zip(&branches)
            .enumerate()
            .map(|(i, (e, (sha, source)))| Merged {
                entry: e.clone(),
                sha: sha.clone(),
                source: *source,
                merged: How::from_word(
                    recipe.get(&format!("entry.{}.merged", i + 1)).map_or("", String::as_str),
                ),
            })
            .collect();
        report.worktree = worktree_of(git, &tag)?.map(|w| w.path);
        report.commit = git.commit(&tag)?;
        report.tag_sha = Some(tag_sha);
        report.tag = Some(tag.clone());
        report.unchanged = true;
        compare(git, &mut report, &tag, reproduces)?;
        return Ok(report);
    }

    let n = next_number(&tags, &prefix).map_err(AssembleError)?;
    let tag = format!("{prefix}{n}");
    let worktree = worktree_for(ask, n)?;
    git.share_rerere()?;
    git.add_worktree(&worktree, &base_sha)?;
    report.worktree = Some(worktree.display().to_string());
    for (entry, (sha, source)) in entries.iter().zip(&branches) {
        let message = format!("rhizoid assemble {tag}: merge {} ({})", entry.branch, entry.purpose);
        let merged = match git.merge(&worktree, sha, &message)? {
            MergeResult::Merged { by_rerere: true } => How::RerereResolved,
            MergeResult::Merged { by_rerere: false } => How::Merged,
            MergeResult::AlreadyContained => How::AlreadyIn,
            MergeResult::Conflict { paths } => {
                report.conflict = Some(conflict(git, &base_sha, &report.entries, entry, paths)?);
                return Ok(report);
            }
        };
        report.entries.push(Merged {
            entry: entry.clone(),
            sha: sha.clone(),
            source: *source,
            merged,
        });
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

/// End a tag's worktree: unlock and remove it; refused when it has changes, unless `force`. Its folder.
pub fn done(git: &impl GitPort, tag: &str, force: bool) -> Result<String, AssembleError> {
    let worktree = worktree_of(git, tag)?.ok_or_else(|| {
        AssembleError(format!("no worktree of {tag} (rhizoid assemble locks each one it tags)"))
    })?;
    if worktree.clean != Some(true) && !force {
        return Err(AssembleError(format!(
            "refused: {} has changes beyond {tag} (or could not be read) - --force removes it anyway",
            worktree.path
        )));
    }
    git.remove_worktree(Path::new(&worktree.path), force)?;
    Ok(worktree.path)
}

/// The worktree assemble locked for `tag`, if one is left.
fn worktree_of(git: &impl GitPort, tag: &str) -> Result<Option<Worktree>, AssembleError> {
    Ok(git
        .worktrees()?
        .into_iter()
        .find(|w| w.lock_reason.as_deref().and_then(assembled_for) == Some(tag)))
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
    Some(
        message
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

/// The newest existing tag whose recipe has this base and these branches at these commits, in this order: its
/// name, its object's sha and its recipe.
#[allow(clippy::type_complexity)]
fn same_recipe(
    git: &impl GitPort,
    tags: &[String],
    base_sha: &str,
    entries: &[Entry],
    branches: &[(String, BranchSource)],
) -> Result<Option<(String, String, BTreeMap<String, String>)>, AssembleError> {
    let mut found: Option<(u64, String, String, BTreeMap<String, String>)> = None;
    for tag in tags {
        let Some(n) = tag.rsplit('/').next().and_then(|n| n.parse::<u64>().ok()) else { continue };
        let (object, message) = git.tag_object(tag)?;
        let Some(recipe) = recipe_of(&message) else { continue };
        let same = recipe.get("base_sha").map(String::as_str) == Some(base_sha)
            && !recipe.contains_key(&format!("entry.{}.branch", entries.len() + 1))
            && entries.iter().zip(branches).enumerate().all(|(i, (e, (sha, _)))| {
                recipe.get(&format!("entry.{}.branch", i + 1)) == Some(&e.branch)
                    && recipe.get(&format!("entry.{}.sha", i + 1)) == Some(sha)
            });
        if same && found.as_ref().map_or(true, |(best, ..)| n > *best) {
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

/// The entries, each known-good one's tested commit resolved to a full sha: it must be a commit's sha (or an
/// unambiguous prefix of one), never a name that moves (HEAD, a branch, a tag).
fn entries_with_tested_shas(
    git: &impl GitPort,
    entries: &[Entry],
) -> Result<Vec<Entry>, AssembleError> {
    entries
        .iter()
        .map(|e| match &e.state {
            State::Trying => Ok(e.clone()),
            State::KnownGood { test_run, tested_commit } => {
                let is_sha = (4..=40).contains(&tested_commit.len())
                    && tested_commit.bytes().all(|b| b.is_ascii_hexdigit());
                let full = if is_sha { git.commit(tested_commit)? } else { None };
                let full = full.ok_or_else(|| {
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

fn worktree_for(ask: &Ask, n: u32) -> Result<PathBuf, AssembleError> {
    let path = match ask.worktree {
        Some(w) => worktree_in_home(ask.home, w).map_err(AssembleError)?,
        None => {
            default_worktree(ask.home, ask.device, ask.list_name, &ask.list.base, n, |p| p.exists())
        }
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
    let mut m =
        format!("{TAG_MESSAGE_START}{}/{} onto {}\n\n", report.device, report.list, report.base);
    for (k, v) in report.recipe() {
        m.push_str(&format!("{k}={v}\n"));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(lists: &str) -> Result<Lists, String> {
        Lists::parse(&format!("device = \"walleye\"\n{lists}"))
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
    fn what_the_scheme_or_a_tag_cannot_carry_is_refused() {
        let entry = |branch: &str, rest: &str| {
            parse(&format!("{ONE}branch = \"{branch}\"\nfor = \"x\"\n{rest}"))
        };
        assert!(entry("wip/x", "state = \"trying\"").unwrap_err().contains("topic/ITEM"));
        assert!(entry("topic/a/b", "state = \"trying\"").is_err());
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
    fn the_next_number_follows_the_highest_tag_of_the_same_prefix() {
        let p = tag_prefix("dev", "two", "v1");
        let tags = |t: &[&str]| t.iter().map(|t| t.to_string()).collect::<Vec<String>>();
        assert_eq!(next_number(&[], &p), Ok(1));
        let some = tags(&[
            "int/dev/two/v1/1",
            "int/dev/two/v1/7",
            "int/dev/two/v1/x",
            "int/dev/two/v1/+9",
            "int/dev/two/v2/9",
        ]);
        assert_eq!(next_number(&some, &p), Ok(8));
        assert!(next_number(&tags(&["int/dev/two/v1/4294967295"]), &p).is_err());
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
    fn the_example_list_file_reads() {
        let l = Lists::parse(include_str!("../../../examples/topics.toml")).unwrap();
        assert_eq!(l.device, "walleye");
        assert_eq!(l.lists.len(), 2);
    }

    #[test]
    fn the_default_worktree_is_the_first_free_name() {
        let home = Path::new("/w");
        let taken = |p: &Path| p == Path::new("/w/d-l-b-1") || p == Path::new("/w/d-l-b-1-2");
        assert_eq!(default_worktree(home, "d", "l", "b", 1, taken), PathBuf::from("/w/d-l-b-1-3"));
        assert_eq!(default_worktree(home, "d", "l", "b", 2, taken), PathBuf::from("/w/d-l-b-2"));
    }
}
