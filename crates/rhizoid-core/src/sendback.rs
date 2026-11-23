//! `send-back`: one branch made into a draft of its next series version for its upstream - rebased onto the
//! target given, cut with `git format-patch --cover-letter -vN --base`, compared with the version last sent
//! (`--range-diff`), and checked by the commands the user configures - for any repository.
//!
//! A version exists once it is sent: the author tags the signed tip he sent as the version tag (`{branch}/vN`),
//! and the next draft is numbered one past those tags. A draft has its own tag (`{branch}/vN-draft`) and send
//! branch, and a draft run again for the same N replaces them: none of them was ever pushed.
//!
//! Every name and every check is the user's (decision 2026-11-23-rhizoid-stays-general): templates and the check
//! and recipient commands come from the config; a Linux kernel tree's checks are one example, in
//! examples/kernel-send.toml. It never signs, re-authors, sends or pushes, and claims nothing in the cover
//! letter: a changelog skeleton and a marked blank for the testing paragraph, for the author to fill.

use crate::assemble::next_number;
use crate::ports::{CommandPort, FormatPatch, GitPort, PortError, RebaseResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What the message of every draft tag send-back writes starts with.
pub const VERSION_MESSAGE_START: &str = "rhizoid send-back: ";

/// The send branch when the config names none: `send/LEAF/vN` (`send/panel/v2` for `topic/panel`).
pub const DEFAULT_SEND_BRANCH: &str = "send/{leaf}/v{n}";

/// The tag of a version sent, when the config names none: `BRANCH/vN` (`topic/panel/v2`). The author makes it.
pub const DEFAULT_VERSION_TAG: &str = "{branch}/v{n}";

/// The tag of a draft, when the config names none: `BRANCH/vN-draft`.
pub const DEFAULT_DRAFT_TAG: &str = "{branch}/v{n}-draft";

/// The cover letter's testing line when the config names none: a marked blank, never filled by rhizoid.
pub const DEFAULT_TESTING: &str = "Testing: <to fill>";

/// rhizoid's config file: what it reads is its `[send]` section.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The send-back section.
    #[serde(default)]
    pub send: SendConfig,
}

/// How send-back names what it makes, and what it runs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendConfig {
    /// The send branch: `{branch}` (the whole name), `{leaf}` (its last part) and one `{n}`.
    #[serde(default = "default_send_branch")]
    pub branch: String,
    /// The tag of a version sent (the author makes it), the same places: versions are numbered past these.
    #[serde(default = "default_version_tag")]
    pub tag: String,
    /// The tag of a draft, the same places.
    #[serde(default = "default_draft_tag")]
    pub draft_tag: String,
    /// The cover letter's testing line: a marked blank for the author.
    #[serde(default = "default_testing")]
    pub testing: String,
    /// The checks, run in order on every draft.
    #[serde(default)]
    pub check: Vec<Step>,
    /// The command whose output lines are the recipients.
    pub recipients: Option<Step>,
}

impl Default for SendConfig {
    fn default() -> Self {
        SendConfig {
            branch: default_send_branch(),
            tag: default_version_tag(),
            draft_tag: default_draft_tag(),
            testing: default_testing(),
            check: Vec::new(),
            recipients: None,
        }
    }
}

fn default_send_branch() -> String {
    DEFAULT_SEND_BRANCH.to_string()
}

fn default_version_tag() -> String {
    DEFAULT_VERSION_TAG.to_string()
}

fn default_draft_tag() -> String {
    DEFAULT_DRAFT_TAG.to_string()
}

fn default_testing() -> String {
    DEFAULT_TESTING.to_string()
}

fn yes() -> bool {
    true
}

/// A command the user configures: argv, run in the rebased worktree, the patch files appended when `patches`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Its name in the report.
    pub name: String,
    /// The program and its arguments.
    pub run: Vec<String>,
    /// Give it the patch files (the cover letter left out), after its own arguments.
    #[serde(default)]
    pub patches: bool,
    /// Whether an exit but 0 fails the draft; `false`: its findings are reported, in SEND-NOTES, and count for
    /// nothing (what is expected of every draft, like a missing sign-off before the author signs).
    #[serde(default = "yes")]
    pub fails: bool,
}

impl Config {
    /// Read a config file (TOML); its templates and steps checked.
    pub fn parse(text: &str) -> Result<Config, String> {
        let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        template(&config.send.branch, "send.branch")?;
        template(&config.send.tag, "send.tag")?;
        template(&config.send.draft_tag, "send.draft_tag")?;
        if config.send.tag == config.send.draft_tag {
            return Err("send.tag and send.draft_tag must differ: a draft is not a version sent"
                .to_string());
        }
        if config.send.testing.contains('\n') {
            return Err("send.testing is one line".to_string());
        }
        for step in config.send.check.iter().chain(&config.send.recipients) {
            if step.run.is_empty() || step.name.trim().is_empty() {
                return Err(format!("a step needs a name and a run: {step:?}"));
            }
        }
        Ok(config)
    }
}

/// A name template: `{branch}`, `{leaf}` and exactly one `{n}`, kept apart from the others.
fn template(text: &str, what: &str) -> Result<(), String> {
    let mut rest = text;
    let mut numbers = 0;
    while let Some(open) = rest.find('{') {
        let close = rest[open..]
            .find('}')
            .ok_or_else(|| format!("{what} {text:?}: a `{{` is not closed"))?;
        match &rest[open + 1..open + close] {
            "n" => numbers += 1,
            "branch" | "leaf" => {}
            other => return Err(format!("{what} {text:?}: {{{other}}} is not branch, leaf or n")),
        }
        rest = &rest[open + close + 1..];
    }
    if rest.contains('}') || numbers != 1 || text.contains("}{n}") || text.contains("{n}{") {
        return Err(format!(
            "{what} {text:?} needs exactly one {{n}}, with text between it and the others"
        ));
    }
    Ok(())
}

/// A template's text before and after its number, for a branch.
fn around_n(template: &str, branch: &str) -> (String, String) {
    let leaf = branch.rsplit('/').next().unwrap_or(branch);
    let text = template.replace("{branch}", branch).replace("{leaf}", leaf);
    let (before, after) = text.split_once("{n}").expect("a checked template has one {n}");
    (before.to_string(), after.to_string())
}

/// A template's name for a branch and a number.
fn named(template: &str, branch: &str, n: u32) -> String {
    let (before, after) = around_n(template, branch);
    format!("{before}{n}{after}")
}

/// What send-back is asked.
pub struct Ask<'a> {
    /// The branch to send.
    pub branch: &'a str,
    /// What to rebase it onto: the target tree's ref (a maintainer's `for-next`, a `next-YYYYMMDD` tag).
    pub onto: &'a str,
    /// The folder the draft goes in: new, empty, or this same draft's from an earlier run.
    pub out: &'a Path,
    /// rhizoid's worktree home for this clone.
    pub home: &'a Path,
    /// The names and steps.
    pub config: &'a SendConfig,
    /// The commit the series was tested on: a commit's sha, passed in, never guessed.
    pub tested: Option<&'a str>,
    /// The fork's id among the caller's upstreams, for `carried-in=FORK/BRANCH`.
    pub fork: Option<&'a str>,
}

/// One step's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepRun {
    /// The step's name.
    pub name: String,
    /// Its exit code; `None` when a signal ended it.
    pub exit: Option<i32>,
    /// Whether its exit counts: `false` for a step whose findings are only reported.
    pub fails: bool,
    /// What it printed.
    pub output: String,
}

impl StepRun {
    fn failed(&self) -> bool {
        self.fails && self.exit != Some(0)
    }
}

/// Where the rebase stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    /// The commit it could not replay.
    pub commit: String,
    /// Its subject.
    pub subject: String,
    /// The paths in conflict.
    pub paths: Vec<String>,
}

/// What a run did. Printed as `key=value` lines or JSON; `carried-in`, `local-ref` and `file` are the record
/// keys a caller's upstream item takes as they are. A draft is no version yet: it says `draft=vN`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The branch sent.
    pub branch: String,
    /// Its commit.
    pub branch_sha: String,
    /// The target it was rebased onto, as given.
    pub base: String,
    /// The target's commit: the series' `--base`.
    pub base_sha: String,
    /// The tested commit, as a full sha.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tested: Option<String>,
    /// The version this is a draft of (`vN`): one past the versions sent.
    pub draft: String,
    /// The draft's tag, made only when the draft is ready (rebased, cut, every counted check passed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_tag: Option<String>,
    /// `FORK/BRANCH`, when the fork's id was given.
    #[serde(rename = "carried-in", skip_serializing_if = "Option::is_none")]
    pub carried_in: Option<String>,
    /// The branch, as an upstream item names it.
    #[serde(rename = "local-ref")]
    pub local_ref: String,
    /// The send branch, kept with the draft tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_branch: Option<String>,
    /// The folder the draft is in.
    pub file: String,
    /// An earlier draft of the same version was replaced: its tag, its send branch, its folder.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub replaced: bool,
    /// Commits of the branch the rebase dropped: the target already holds them.
    #[serde(skip_serializing_if = "is_zero")]
    pub dropped: u64,
    /// The files written, the cover letter first.
    pub patches: Vec<String>,
    /// The version sent it is compared with (`--range-diff`), when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_diff: Option<String>,
    /// The checks, in order.
    pub checks: Vec<StepRun>,
    /// The recipients, one per line the recipients command printed.
    pub recipients: Vec<String>,
    /// Where the rebase stopped; nothing else was made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<Conflict>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

impl Report {
    /// The draft is ready: rebased, cut, every counted check passed, the draft tagged.
    pub fn ok(&self) -> bool {
        self.conflict.is_none()
            && !self.checks.iter().any(StepRun::failed)
            && self.draft_tag.is_some()
    }

    /// Every field as `key=value` pairs.
    pub fn key_values(&self) -> Vec<(String, String)> {
        let mut kv = Vec::new();
        let mut put = |k: &str, v: &str| kv.push((k.to_string(), v.to_string()));
        put("branch", &self.branch);
        put("branch_sha", &self.branch_sha);
        put("base", &self.base);
        put("base_sha", &self.base_sha);
        if let Some(t) = &self.tested {
            put("tested", t);
        }
        put("draft", &self.draft);
        if let Some(t) = &self.draft_tag {
            put("draft_tag", t);
        }
        if let Some(c) = &self.carried_in {
            put("carried-in", c);
        }
        put("local-ref", &self.local_ref);
        if let Some(s) = &self.send_branch {
            put("send_branch", s);
        }
        put("file", &self.file);
        if self.replaced {
            put("replaced", "yes");
        }
        if self.dropped > 0 {
            put("dropped", &self.dropped.to_string());
        }
        for (i, p) in self.patches.iter().enumerate() {
            put(&format!("patch.{}", i + 1), p);
        }
        put("range_diff", self.range_diff.as_deref().unwrap_or("none"));
        for (i, c) in self.checks.iter().enumerate() {
            put(&format!("check.{}.name", i + 1), &c.name);
            let exit = c.exit.map_or("signal".to_string(), |e| e.to_string());
            put(&format!("check.{}.exit", i + 1), &exit);
            if !c.fails {
                put(&format!("check.{}.counts", i + 1), "no");
            }
        }
        for (i, r) in self.recipients.iter().enumerate() {
            put(&format!("recipient.{}", i + 1), r);
        }
        if let Some(c) = &self.conflict {
            put("conflict.commit", &c.commit);
            put("conflict.subject", &c.subject);
            for (i, p) in c.paths.iter().enumerate() {
                put(&format!("conflict.path.{}", i + 1), p);
            }
        }
        kv
    }
}

/// Why send-back did not run; nothing it says was made, unless it says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError(pub String);

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<PortError> for SendError {
    fn from(e: PortError) -> Self {
        SendError(e.0)
    }
}

/// What a commit missing from the clone's history means, and how to get it.
const MISSING_HISTORY: &str = "is not in this clone's history - a shallow clone? Deepen it (git fetch --deepen=N, or --shallow-since=DATE) or fetch the ref, then run again";

/// An earlier draft of the same version, found and proven to be this draft's: what a re-run replaces.
struct Earlier {
    /// The draft tag exists (its message says this branch and this draft).
    tag: bool,
    /// The send branch exists, at the draft tag's commit.
    send: bool,
    /// The `--out` folder holds this draft's files.
    out: bool,
}

impl Earlier {
    fn any(&self) -> bool {
        self.tag || self.send || self.out
    }
}

/// Draft the next version of one branch: read and check everything, the clone's history included, before
/// anything is written; rebase onto the target in a fresh worktree (a conflict stops it, and nothing is kept);
/// cut the series with its range-diff against the version last sent; run the checks and the recipients command;
/// when every counted check passes, tag the draft and keep its send branch. An earlier draft of the same version
/// is replaced only once the new one is ready: until then it is cut beside it. The worktree always goes.
pub fn send_back(
    git: &impl GitPort,
    cmd: &impl CommandPort,
    ask: &Ask,
) -> Result<Report, SendError> {
    let (branch_sha, _) = git
        .branch(ask.branch)?
        .ok_or_else(|| SendError(format!("no branch {} (locally or on origin)", ask.branch)))?;
    if ask.onto.starts_with('-') {
        return Err(SendError(format!("--onto {}: not a ref", ask.onto)));
    }
    let base_sha = git.commit(ask.onto)?.ok_or_else(|| {
        SendError(format!("--onto {}: no such commit in the clone (fetch it)", ask.onto))
    })?;
    let tested = ask.tested.map(|t| tested_sha(git, t)).transpose()?;
    whole_history(
        git,
        &base_sha,
        &branch_sha,
        &format!("where {} and {} meet", ask.branch, ask.onto),
    )?;

    let (before, after) = around_n(&ask.config.tag, ask.branch);
    let n = next_number(&git.tags(&before)?, &before, &after).map_err(SendError)?;
    let draft_tag = named(&ask.config.draft_tag, ask.branch, n);
    let send = named(&ask.config.branch, ask.branch, n);
    let range_diff = last_sent_range(git, ask, n, &base_sha)?;
    let earlier = earlier_draft(git, ask, n, &draft_tag, &send)?;

    // a re-run cuts beside the earlier draft, and swaps it in only once ready
    let (cut_send, cut_out) = if earlier.any() {
        (format!("{send}-next"), PathBuf::from(format!("{}.next", ask.out.display())))
    } else {
        (send.clone(), ask.out.to_path_buf())
    };
    if git.commit(&format!("refs/heads/{cut_send}"))?.is_some() {
        return Err(SendError(format!(
            "refused: the branch {cut_send} exists - an earlier run's? delete it"
        )));
    }
    if earlier.any() && cut_out.exists() {
        return Err(SendError(format!(
            "refused: {} exists - an earlier run's? remove it",
            cut_out.display()
        )));
    }
    let worktree = ask.home.join(cut_send.replace('/', "-"));
    if worktree.exists() {
        return Err(SendError(format!(
            "refused: {} exists - a worktree in use? remove it",
            worktree.display()
        )));
    }

    let mut report = Report {
        branch: ask.branch.to_string(),
        branch_sha: branch_sha.clone(),
        base: ask.onto.to_string(),
        base_sha: base_sha.clone(),
        tested,
        draft: format!("v{n}"),
        draft_tag: None,
        carried_in: ask.fork.map(|f| format!("{f}/{}", ask.branch)),
        local_ref: ask.branch.to_string(),
        send_branch: None,
        file: cut_out.display().to_string(),
        replaced: false,
        dropped: 0,
        patches: Vec::new(),
        range_diff: range_diff.as_ref().map(|(tag, _)| tag.clone()),
        checks: Vec::new(),
        recipients: Vec::new(),
        conflict: None,
    };

    let created_out = !cut_out.exists();
    git.add_branch_worktree(&worktree, &cut_send, &branch_sha)?;
    let made = Making { n, send: &cut_send, worktree: &worktree, out: &cut_out, range_diff };
    let ready = match make_draft(git, cmd, ask, &mut report, made) {
        Ok(ready) => ready,
        Err(e) => {
            // nothing is kept of a run that could not finish: not its send branch, nor a folder it made
            let _ = drop_send(git, &worktree, &cut_send);
            if created_out {
                let _ = std::fs::remove_dir_all(&cut_out);
            }
            return Err(e);
        }
    };
    let Some(tip) = ready else { return Ok(report) };
    if earlier.any() {
        replace_earlier(
            git,
            ask,
            n,
            &earlier,
            &draft_tag,
            &send,
            &cut_send,
            &cut_out,
            &mut report,
        )?;
    }
    git.tag(&draft_tag, &tip, &draft_message(&report, n))?;
    report.draft_tag = Some(draft_tag);
    report.send_branch = Some(send);
    Ok(report)
}

/// What one run cuts, named before the worktree exists.
struct Making<'a> {
    n: u32,
    send: &'a str,
    worktree: &'a Path,
    out: &'a Path,
    range_diff: Option<(String, String)>,
}

/// From the fresh worktree on: rebase, cut, check; the rebased tip when the draft is ready, `None` when a
/// conflict or a failed check stopped it (its send branch dropped).
fn make_draft(
    git: &impl GitPort,
    cmd: &impl CommandPort,
    ask: &Ask,
    report: &mut Report,
    made: Making,
) -> Result<Option<String>, SendError> {
    let Making { n, send, worktree, out, range_diff } = made;
    let base_sha = report.base_sha.clone();
    if let RebaseResult::Conflict { commit, subject, paths } = git.rebase(worktree, &base_sha)? {
        report.conflict = Some(Conflict { commit, subject, paths });
        drop_send(git, worktree, send)?;
        return Ok(None);
    }
    let tip = git.head(worktree)?;
    let series = git.count(&base_sha, &tip)?;
    if series == 0 {
        return Err(SendError(format!(
            "nothing to send: {} holds every commit of {}",
            ask.onto, ask.branch
        )));
    }
    report.dropped = git.count(&base_sha, &report.branch_sha)?.saturating_sub(series);
    std::fs::create_dir_all(out).map_err(|e| SendError(format!("{}: {e}", out.display())))?;
    report.patches = git.format_patch(
        worktree,
        &FormatPatch {
            range: format!("{base_sha}..{tip}"),
            version: n,
            base: base_sha.clone(),
            range_diff: range_diff.map(|(_, range)| range),
            out: out.to_path_buf(),
        },
    )?;
    // with --cover-letter, git writes the cover letter first
    let Some((cover, patch_files)) = report.patches.split_first() else {
        return Err(SendError("git format-patch wrote nothing".to_string()));
    };
    cover_skeleton(cover, n, &ask.config.testing)?;
    let patch_files = patch_files.to_vec();
    for step in &ask.config.check {
        report.checks.push(run_step(cmd, step, &patch_files, worktree)?.0);
    }
    if let Some(step) = &ask.config.recipients {
        let (run, stdout) = run_step(cmd, step, &patch_files, worktree)?;
        if run.exit == Some(0) {
            report.recipients = stdout
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
        } else {
            report.checks.push(StepRun { fails: true, ..run });
        }
    }
    write_notes(out, report)?;
    if report.checks.iter().any(StepRun::failed) {
        drop_send(git, worktree, send)?;
        return Ok(None);
    }
    git.remove_worktree(worktree)?;
    Ok(Some(tip))
}

/// The earlier draft of version `n`, if any: each part of it proven to be this branch's draft, or refused - a
/// tag, a branch or a folder that may be someone's work is never replaced.
fn earlier_draft(
    git: &impl GitPort,
    ask: &Ask,
    n: u32,
    draft_tag: &str,
    send: &str,
) -> Result<Earlier, SendError> {
    let tag_commit = git.commit(&format!("refs/tags/{draft_tag}"))?;
    if tag_commit.is_some() {
        let (_, message) = git.tag_object(draft_tag)?;
        let says = |k: &str, v: &str| message.lines().any(|l| l == format!("{k}={v}"));
        if !message.starts_with(VERSION_MESSAGE_START)
            || !says("branch", ask.branch)
            || !says("draft", &format!("v{n}"))
        {
            return Err(SendError(format!("refused: the tag {draft_tag} is not a draft of {} v{n} rhizoid made - move it away", ask.branch)));
        }
    }
    let send_commit = git.commit(&format!("refs/heads/{send}"))?;
    if let Some(at) = &send_commit {
        if tag_commit.as_ref() != Some(at) {
            return Err(SendError(format!(
                "refused: the branch {send} is not at the draft {draft_tag} - work on it? Tag it as the version sent, or delete it"
            )));
        }
    }
    let send_worktree = ask.home.join(send.replace('/', "-"));
    if send_worktree.exists() {
        return Err(SendError(format!(
            "refused: {} exists - a worktree on the draft, in use? remove it",
            send_worktree.display()
        )));
    }
    Ok(Earlier {
        tag: tag_commit.is_some(),
        send: send_commit.is_some(),
        out: out_holds_this_draft(ask.out, ask.branch, n)?,
    })
}

/// Replace the earlier draft with the one just cut beside it: its tag and send branch, and in `--out` its own
/// files (SEND-NOTES and the series), nothing else.
#[allow(clippy::too_many_arguments)]
fn replace_earlier(
    git: &impl GitPort,
    ask: &Ask,
    n: u32,
    earlier: &Earlier,
    draft_tag: &str,
    send: &str,
    cut_send: &str,
    cut_out: &Path,
    report: &mut Report,
) -> Result<(), SendError> {
    if earlier.tag {
        git.delete_tag(draft_tag)?;
    }
    if earlier.send {
        git.delete_branch(send)?;
    }
    git.rename_branch(cut_send, send)?;
    let io = |e: std::io::Error, p: &Path| SendError(format!("{}: {e}", p.display()));
    if earlier.out {
        for entry in std::fs::read_dir(ask.out).map_err(|e| io(e, ask.out))? {
            let path = entry.map_err(|e| io(e, ask.out))?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "SEND-NOTES"
                || (name.starts_with(&format!("v{n}-")) && name.ends_with(".patch"))
            {
                std::fs::remove_file(&path).map_err(|e| io(e, &path))?;
            }
        }
    }
    std::fs::create_dir_all(ask.out).map_err(|e| io(e, ask.out))?;
    let mut moved = Vec::new();
    for file in &report.patches {
        let from = PathBuf::from(file);
        let to = ask.out.join(from.file_name().unwrap_or_default());
        std::fs::rename(&from, &to).map_err(|e| io(e, &from))?;
        moved.push(to.display().to_string());
    }
    let notes = cut_out.join("SEND-NOTES");
    std::fs::rename(&notes, ask.out.join("SEND-NOTES")).map_err(|e| io(e, &notes))?;
    std::fs::remove_dir(cut_out).map_err(|e| io(e, cut_out))?;
    report.patches = moved;
    report.file = ask.out.display().to_string();
    report.replaced = true;
    Ok(())
}

/// The clone's history from `from` up to `to` is whole: they meet, and no shallow boundary lies between - a
/// shallow clone could otherwise show a wrong, too deep meeting point.
fn whole_history(git: &impl GitPort, from: &str, to: &str, what: &str) -> Result<(), SendError> {
    let base = git.merge_base(from, to)?;
    let cut = match &base {
        Some(base) => {
            git.shallow_boundary_between(base, to)? || git.shallow_boundary_between(base, from)?
        }
        None => true,
    };
    if cut {
        Err(SendError(format!("{what} {MISSING_HISTORY}")))
    } else {
        Ok(())
    }
}

/// The tested commit as a full sha: a commit's sha (or a prefix of one), never a name that moves.
fn tested_sha(git: &impl GitPort, tested: &str) -> Result<String, SendError> {
    git.commit_by_sha(tested)?.ok_or_else(|| {
        SendError(format!(
            "--tested {tested} is not a commit's sha in the clone (a name that moves, like HEAD or a branch, cannot say what was tested)"
        ))
    })
}

/// Whether `out` holds this same draft from an earlier run (its SEND-NOTES says so). A folder that is new or
/// empty is fine; any other is refused - a draft never writes where it did not write before.
fn out_holds_this_draft(out: &Path, branch: &str, n: u32) -> Result<bool, SendError> {
    let empty = match std::fs::read_dir(out).map(|mut entries| entries.next().is_none()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(SendError(format!("{}: {e}", out.display()))),
        Ok(empty) => empty,
    };
    if empty {
        return Ok(false);
    }
    let notes = std::fs::read_to_string(out.join("SEND-NOTES")).unwrap_or_default();
    let says = |k: &str, v: &str| notes.lines().any(|l| l == format!("{k}={v}"));
    if says("branch", branch) && says("draft", &format!("v{n}")) {
        Ok(true)
    } else {
        Err(SendError(format!(
            "refused: {} is not empty, and not this draft's (v{n} of {branch}) - a draft goes in a folder of its own",
            out.display()
        )))
    }
}

/// The version last sent (`{branch}/v{n-1}`) and its commits, for `--range-diff`: from the base its draft was
/// cut on when the sent tip is that draft's length above it, else from where it meets the target. Refused when
/// the clone's history cannot hold it; none for v1.
fn last_sent_range(
    git: &impl GitPort,
    ask: &Ask,
    n: u32,
    base_sha: &str,
) -> Result<Option<(String, String)>, SendError> {
    if n < 2 {
        return Ok(None);
    }
    let sent = named(&ask.config.tag, ask.branch, n - 1);
    let sent_sha = git
        .commit(&format!("refs/tags/{sent}"))?
        .ok_or_else(|| SendError(format!("the version last sent, {sent}, {MISSING_HISTORY}")))?;
    let draft = named(&ask.config.draft_tag, ask.branch, n - 1);
    let mut draft_base = None;
    if let Some(draft_sha) = git.commit(&format!("refs/tags/{draft}"))? {
        let (_, message) = git.tag_object(&draft)?;
        let recipe: BTreeMap<&str, &str> =
            message.lines().filter_map(|l| l.split_once('=')).collect();
        if let Some(base) = recipe
            .get("base_sha")
            .filter(|b| b.len() == 40 && b.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            let same_length = git.merge_base(base, &sent_sha)?.as_deref() == Some(*base)
                && git.count(base, &sent_sha)? == git.count(base, &draft_sha)?;
            if same_length {
                draft_base = Some(base.to_string());
            }
        }
    }
    let base = match draft_base {
        Some(base) => base,
        None => {
            whole_history(
                git,
                base_sha,
                &sent_sha,
                &format!("where {sent} and {} meet", ask.onto),
            )?;
            git.merge_base(&sent_sha, base_sha)?.expect("whole_history found where they meet")
        }
    };
    Ok(Some((sent.clone(), format!("{base}..{sent_sha}"))))
}

/// Under the cover letter's blurb: a changelog skeleton and the testing line, marked blanks for the author.
fn cover_skeleton(cover: &str, n: u32, testing: &str) -> Result<(), SendError> {
    let text = std::fs::read_to_string(cover).map_err(|e| SendError(format!("{cover}: {e}")))?;
    let heading =
        if n > 1 { format!("Changes since v{}:", n - 1) } else { format!("Changes in v{n}:") };
    let text = text.replacen(
        "*** BLURB HERE ***",
        &format!("*** BLURB HERE ***\n\n{heading}\n- \n\n{testing}\n"),
        1,
    );
    std::fs::write(cover, text).map_err(|e| SendError(format!("{cover}: {e}")))
}

fn run_step(
    cmd: &impl CommandPort,
    step: &Step,
    patches: &[String],
    dir: &Path,
) -> Result<(StepRun, String), SendError> {
    let mut argv = step.run.clone();
    if step.patches {
        argv.extend(patches.iter().cloned());
    }
    let (exit, stdout, stderr) = cmd.run(&argv, dir)?;
    let output = if stderr.is_empty() { stdout.clone() } else { format!("{stdout}{stderr}") };
    Ok((StepRun { name: step.name.clone(), exit, fails: step.fails, output }, stdout))
}

/// SEND-NOTES beside the patches: what the draft is, what it is based on and was tested on, and every check's
/// output (a reported-only check's too, marked).
fn write_notes(out: &Path, report: &Report) -> Result<(), SendError> {
    let mut notes = format!(
        "branch={}\nbranch_sha={}\nbase={}\nbase_sha={}\ntested={}\ndraft={}\n",
        report.branch,
        report.branch_sha,
        report.base,
        report.base_sha,
        report.tested.as_deref().unwrap_or("not given"),
        report.draft
    );
    for c in &report.checks {
        let exit = c.exit.map_or("signal".to_string(), |e| e.to_string());
        let counts = if c.fails { "" } else { ", reported only" };
        notes.push_str(&format!("\n== check {} (exit {exit}{counts})\n{}", c.name, c.output));
    }
    let path: PathBuf = out.join("SEND-NOTES");
    std::fs::write(&path, notes).map_err(|e| SendError(format!("{}: {e}", path.display())))
}

fn draft_message(report: &Report, n: u32) -> String {
    let mut m =
        format!("{VERSION_MESSAGE_START}{} draft of v{n} onto {}\n\n", report.branch, report.base);
    m.push_str(&format!(
        "branch={}\nbranch_sha={}\nbase={}\nbase_sha={}\ndraft=v{n}\n",
        report.branch, report.branch_sha, report.base, report.base_sha
    ));
    if let Some(t) = &report.tested {
        m.push_str(&format!("tested={t}\n"));
    }
    m
}

/// End a draft that will not be kept: its worktree, mid-rebase or not, and its branch.
fn drop_send(git: &impl GitPort, worktree: &Path, send: &str) -> Result<(), SendError> {
    git.remove_worktree(worktree)?;
    git.delete_branch(send)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_template_has_one_number_and_known_places() {
        assert!(template("send/{leaf}/v{n}", "b").is_ok());
        assert!(template("{branch}/v{n}-draft", "t").is_ok());
        assert!(template("x/{item}/{n}", "t").unwrap_err().contains("not branch, leaf or n"));
        assert!(template("x/{leaf}", "t").is_err());
        assert!(template("x/{leaf}{n}", "t").is_err());
    }

    #[test]
    fn the_defaults_name_a_topics_send_branch_draft_and_version() {
        assert_eq!(named(DEFAULT_SEND_BRANCH, "topic/panel", 2), "send/panel/v2");
        assert_eq!(named(DEFAULT_VERSION_TAG, "topic/panel", 2), "topic/panel/v2");
        assert_eq!(named(DEFAULT_DRAFT_TAG, "topic/panel", 2), "topic/panel/v2-draft");
    }

    #[test]
    fn the_example_kernel_config_reads() {
        let c = Config::parse(include_str!("../../../examples/kernel-send.toml")).unwrap();
        assert!(
            c.send.check.iter().any(|s| !s.fails),
            "the draft's expected findings are reported only"
        );
        assert!(c.send.testing.contains("evidence"));
        assert!(c.send.recipients.is_some());
    }

    #[test]
    fn a_config_reads_its_steps_and_refuses_what_cannot_hold() {
        let c = Config::parse(
            "[send]\n[[send.check]]\nname = \"style\"\nrun = [\"lint\", \"--strict\"]\npatches = true\n\
             [send.recipients]\nname = \"who\"\nrun = [\"who\"]\npatches = true\n",
        )
        .unwrap();
        assert_eq!(c.send.check[0].run, ["lint", "--strict"]);
        assert!(c.send.check[0].fails, "a check counts unless it says not");
        assert_eq!(c.send.branch, DEFAULT_SEND_BRANCH);
        assert!(Config::parse("[send]\n[[send.check]]\nname = \"x\"\nrun = []\n").is_err());
        assert!(Config::parse("[send]\ntag = \"{branch}/v{n}\"\ndraft_tag = \"{branch}/v{n}\"\n")
            .is_err());
        assert!(Config::parse("").unwrap().send.check.is_empty(), "no config: no checks");
        assert!(Config::parse("[sned]\n").is_err(), "a misspelled section");
    }
}
