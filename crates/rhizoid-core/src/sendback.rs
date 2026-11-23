//! `send-back`: one branch made into a series version for its upstream - rebased onto the target given, cut
//! with `git format-patch --cover-letter -vN --base`, compared with the version before (`--range-diff`), and
//! checked by the commands the user configures - for any repository.
//!
//! Every name and every check is the user's (decision 2026-11-23-rhizoid-stays-general): the send branch and
//! version tag templates, and the check and recipient commands, come from the config; a kernel's
//! `get_maintainer.pl`, `checkpatch.pl --strict` and `W=1` build are one example. It never signs, re-authors,
//! sends or pushes, and claims nothing in the cover letter beyond a changelog skeleton to fill (the author sends).

use crate::assemble::next_number;
use crate::ports::{CommandPort, FormatPatch, GitPort, PortError, RebaseResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What the message of every version tag send-back writes starts with.
pub const VERSION_MESSAGE_START: &str = "rhizoid send-back: ";

/// The send branch when the config names none: `send/LEAF/vN` (`send/panel/v2` for `topic/panel`).
pub const DEFAULT_SEND_BRANCH: &str = "send/{leaf}/v{n}";

/// The version tag when the config names none: `BRANCH/vN` (`topic/panel/v2`).
pub const DEFAULT_VERSION_TAG: &str = "{branch}/v{n}";

/// rhizoid's config file: what it reads is its `[send]` section.
#[derive(Debug, Clone, Default, Deserialize)]
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
    /// The version tag, the same places.
    #[serde(default = "default_version_tag")]
    pub tag: String,
    /// The checks, run in order on every version; any exit but 0 is a failed check.
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
}

impl Config {
    /// Read a config file (TOML); its templates and steps checked.
    pub fn parse(text: &str) -> Result<Config, String> {
        let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        template(&config.send.branch, "send.branch")?;
        template(&config.send.tag, "send.tag")?;
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

/// What send-back is asked.
pub struct Ask<'a> {
    /// The branch to send.
    pub branch: &'a str,
    /// What to rebase it onto: the target tree's ref (a maintainer's `for-next`).
    pub onto: &'a str,
    /// The folder the series goes in: new, or empty.
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
    /// What it printed.
    pub output: String,
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

/// What a run did. Printed as `key=value` lines or JSON; `version`, `carried-in`, `local-ref` and `file` are
/// the record keys a caller's upstream item takes as they are.
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
    /// The series version (`vN`).
    pub version: String,
    /// `FORK/BRANCH`, when the fork's id was given.
    #[serde(rename = "carried-in", skip_serializing_if = "Option::is_none")]
    pub carried_in: Option<String>,
    /// The version tag, made only when the series is ready (rebased, cut, every check passed).
    #[serde(rename = "local-ref", skip_serializing_if = "Option::is_none")]
    pub local_ref: Option<String>,
    /// The send branch, kept with the tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_branch: Option<String>,
    /// The folder the series is in.
    pub file: String,
    /// The files written, the cover letter first.
    pub patches: Vec<String>,
    /// The version compared with (`--range-diff`), when there is one with a recipe.
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

impl Report {
    /// The series is ready: rebased, cut, every check passed, the version tagged.
    pub fn ok(&self) -> bool {
        self.conflict.is_none()
            && self.checks.iter().all(|c| c.exit == Some(0))
            && self.local_ref.is_some()
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
        put("version", &self.version);
        if let Some(c) = &self.carried_in {
            put("carried-in", c);
        }
        if let Some(r) = &self.local_ref {
            put("local-ref", r);
        }
        if let Some(s) = &self.send_branch {
            put("send_branch", s);
        }
        put("file", &self.file);
        for (i, p) in self.patches.iter().enumerate() {
            put(&format!("patch.{}", i + 1), p);
        }
        put("range_diff", self.range_diff.as_deref().unwrap_or("none"));
        for (i, c) in self.checks.iter().enumerate() {
            put(&format!("check.{}.name", i + 1), &c.name);
            put(
                &format!("check.{}.exit", i + 1),
                &c.exit.map_or("signal".to_string(), |e| e.to_string()),
            );
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

/// Send one branch back: read and check everything; rebase it onto the target in a fresh worktree (a conflict
/// stops it, and nothing is kept); cut the series with its range-diff; run the checks and the recipients
/// command; when every check passes, tag the version and keep the send branch. The worktree always goes.
pub fn send_back(
    git: &impl GitPort,
    cmd: &impl CommandPort,
    ask: &Ask,
) -> Result<Report, SendError> {
    let (branch_sha, _) = git
        .branch(ask.branch)?
        .ok_or_else(|| SendError(format!("no branch {} (locally or on origin)", ask.branch)))?;
    let base_sha = git.commit(ask.onto)?.ok_or_else(|| {
        SendError(format!("--onto {}: no such commit in the clone (fetch it)", ask.onto))
    })?;
    let tested = ask.tested.map(|t| tested_sha(git, t)).transpose()?;
    out_is_free(ask.out)?;

    let (before, after) = around_n(&ask.config.tag, ask.branch);
    let n = next_number(&git.tags(&before)?, &before, &after).map_err(SendError)?;
    let tag = format!("{before}{n}{after}");
    let (send_before, send_after) = around_n(&ask.config.branch, ask.branch);
    let send = format!("{send_before}{n}{send_after}");
    if git.commit(&format!("refs/heads/{send}"))?.is_some() {
        return Err(SendError(format!(
            "refused: the branch {send} exists - delete it, or tag the version it holds"
        )));
    }
    let range_diff = previous_range(git, &before, &after, n)?;
    let worktree = ask.home.join(send.replace('/', "-"));
    if worktree.exists() {
        return Err(SendError(format!("refused: {} exists", worktree.display())));
    }

    let mut report = Report {
        branch: ask.branch.to_string(),
        branch_sha: branch_sha.clone(),
        base: ask.onto.to_string(),
        base_sha: base_sha.clone(),
        tested,
        version: format!("v{n}"),
        carried_in: ask.fork.map(|f| format!("{f}/{}", ask.branch)),
        local_ref: None,
        send_branch: None,
        file: ask.out.display().to_string(),
        patches: Vec::new(),
        range_diff: range_diff.as_ref().map(|(tag, _)| tag.clone()),
        checks: Vec::new(),
        recipients: Vec::new(),
        conflict: None,
    };

    git.add_branch_worktree(&worktree, &send, &branch_sha)?;
    if let RebaseResult::Conflict { commit, subject, paths } = git.rebase(&worktree, &base_sha)? {
        report.conflict = Some(Conflict { commit, subject, paths });
        drop_send(git, &worktree, &send)?;
        return Ok(report);
    }
    let tip = git.head(&worktree)?;
    std::fs::create_dir_all(ask.out)
        .map_err(|e| SendError(format!("{}: {e}", ask.out.display())))?;
    report.patches = git.format_patch(
        &worktree,
        &FormatPatch {
            range: format!("{base_sha}..{tip}"),
            version: n,
            base: base_sha.clone(),
            range_diff: range_diff.map(|(_, range)| range),
            out: ask.out.to_path_buf(),
        },
    )?;
    changelog_skeleton(&report.patches, n)?;
    let patch_files: Vec<String> =
        report.patches.iter().filter(|p| !p.contains("0000-cover-letter")).cloned().collect();
    for step in &ask.config.check {
        report.checks.push(run_step(cmd, step, &patch_files, &worktree)?);
    }
    if let Some(step) = &ask.config.recipients {
        let run = run_step(cmd, step, &patch_files, &worktree)?;
        report.recipients = run
            .output
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
    }
    write_notes(ask.out, &report)?;

    if report.checks.iter().all(|c| c.exit == Some(0)) {
        git.tag(&tag, &tip, &version_message(&report, n))?;
        report.local_ref = Some(tag);
        report.send_branch = Some(send);
        git.remove_worktree(&worktree)?;
    } else {
        drop_send(git, &worktree, &send)?;
    }
    Ok(report)
}

/// The tested commit as a full sha: a commit's sha (or a prefix of one), never a name that moves.
fn tested_sha(git: &impl GitPort, tested: &str) -> Result<String, SendError> {
    git.commit_by_sha(tested)?.ok_or_else(|| {
        SendError(format!(
            "--tested {tested} is not a commit's sha in the clone (a name that moves, like HEAD or a branch, cannot say what was tested)"
        ))
    })
}

/// The series folder must be new or empty: a version never overwrites another.
fn out_is_free(out: &Path) -> Result<(), SendError> {
    match std::fs::read_dir(out).map(|mut entries| entries.next().is_none()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(SendError(format!("{}: {e}", out.display()))),
        Ok(true) => Ok(()),
        Ok(false) => Err(SendError(format!(
            "refused: {} is not empty - a version goes in a folder of its own",
            out.display()
        ))),
    }
}

/// The previous version's tag and its commits (`BASE..TAG`), read from its recipe; none for v1, or when the
/// previous tag has no recipe.
fn previous_range(
    git: &impl GitPort,
    before: &str,
    after: &str,
    n: u32,
) -> Result<Option<(String, String)>, SendError> {
    if n < 2 {
        return Ok(None);
    }
    let prev = format!("{before}{}{after}", n - 1);
    let (_, message) = git.tag_object(&prev)?;
    if !message.starts_with(VERSION_MESSAGE_START) {
        return Ok(None);
    }
    let recipe: BTreeMap<&str, &str> = message.lines().filter_map(|l| l.split_once('=')).collect();
    Ok(recipe.get("base_sha").map(|base| (prev.clone(), format!("{base}..{prev}"))))
}

/// Put a changelog skeleton under the cover letter's blurb, for the author to fill.
fn changelog_skeleton(patches: &[String], n: u32) -> Result<(), SendError> {
    let Some(cover) = patches.iter().find(|p| p.contains("0000-cover-letter")) else {
        return Ok(());
    };
    let text = std::fs::read_to_string(cover).map_err(|e| SendError(format!("{cover}: {e}")))?;
    let heading =
        if n > 1 { format!("Changes since v{}:", n - 1) } else { format!("Changes in v{n}:") };
    let text =
        text.replacen("*** BLURB HERE ***", &format!("*** BLURB HERE ***\n\n{heading}\n- \n"), 1);
    std::fs::write(cover, text).map_err(|e| SendError(format!("{cover}: {e}")))
}

fn run_step(
    cmd: &impl CommandPort,
    step: &Step,
    patches: &[String],
    dir: &Path,
) -> Result<StepRun, SendError> {
    let mut argv = step.run.clone();
    if step.patches {
        argv.extend(patches.iter().cloned());
    }
    let (exit, stdout, stderr) = cmd.run(&argv, dir)?;
    let output = if stderr.is_empty() { stdout } else { format!("{stdout}{stderr}") };
    Ok(StepRun { name: step.name.clone(), exit, output })
}

/// SEND-NOTES beside the patches: what the series is based on and was tested on, and every check's output.
fn write_notes(out: &Path, report: &Report) -> Result<(), SendError> {
    let mut notes = format!(
        "branch={}\nbranch_sha={}\nbase={}\nbase_sha={}\ntested={}\nversion={}\n",
        report.branch,
        report.branch_sha,
        report.base,
        report.base_sha,
        report.tested.as_deref().unwrap_or("not given"),
        report.version
    );
    for c in &report.checks {
        let exit = c.exit.map_or("signal".to_string(), |e| e.to_string());
        notes.push_str(&format!("\n== check {} (exit {exit})\n{}", c.name, c.output));
    }
    let path: PathBuf = out.join("SEND-NOTES");
    std::fs::write(&path, notes).map_err(|e| SendError(format!("{}: {e}", path.display())))
}

fn version_message(report: &Report, n: u32) -> String {
    let mut m = format!("{VERSION_MESSAGE_START}{} v{n} onto {}\n\n", report.branch, report.base);
    m.push_str(&format!(
        "branch={}\nbranch_sha={}\nbase={}\nbase_sha={}\nn={n}\n",
        report.branch, report.branch_sha, report.base, report.base_sha
    ));
    if let Some(t) = &report.tested {
        m.push_str(&format!("tested={t}\n"));
    }
    m
}

/// End a send that will not be kept: its worktree, mid-rebase or not, and its branch.
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
        assert!(template("{branch}/v{n}", "t").is_ok());
        assert!(template("x/{item}/{n}", "t").unwrap_err().contains("not branch, leaf or n"));
        assert!(template("x/{leaf}", "t").is_err());
        assert!(template("x/{leaf}{n}", "t").is_err());
    }

    #[test]
    fn the_defaults_name_a_topics_send_branch_and_version_tag() {
        assert_eq!(
            around_n(DEFAULT_SEND_BRANCH, "topic/panel"),
            ("send/panel/v".to_string(), String::new())
        );
        assert_eq!(
            around_n(DEFAULT_VERSION_TAG, "topic/panel"),
            ("topic/panel/v".to_string(), String::new())
        );
    }

    #[test]
    fn the_example_kernel_config_reads() {
        let c = Config::parse(include_str!("../../../examples/kernel-send.toml")).unwrap();
        assert_eq!(c.send.check.len(), 3);
        assert!(c.send.recipients.is_some());
    }

    #[test]
    fn a_config_reads_its_steps_and_refuses_an_empty_one() {
        let c = Config::parse(
            "[send]\n[[send.check]]\nname = \"style\"\nrun = [\"lint\", \"--strict\"]\npatches = true\n\
             [send.recipients]\nname = \"who\"\nrun = [\"who\"]\npatches = true\n",
        )
        .unwrap();
        assert_eq!(c.send.check[0].run, ["lint", "--strict"]);
        assert_eq!(c.send.branch, DEFAULT_SEND_BRANCH);
        assert!(Config::parse("[send]\n[[send.check]]\nname = \"x\"\nrun = []\n").is_err());
        assert!(Config::parse("").unwrap().send.check.is_empty(), "no config: no checks");
    }
}
