//! `rhizoid send-back BRANCH --onto REF --out DIR` - one branch made into a series version for its upstream:
//! rebased, cut with its cover letter and range-diff, checked by the configured commands, and tagged when every
//! check passes. It ends 0 when the version is tagged, 1 at a conflict or a failed check, 2 when it could not run.

use crate::git_cli::{GitCli, Programs};
use rhizoid_core::sendback::{send_back, Ask as CoreAsk, Config, Report};
use std::path::PathBuf;

/// What send-back was asked, with every default already decided.
pub struct Ask {
    pub branch: String,
    pub onto: String,
    pub out: PathBuf,
    pub repo: PathBuf,
    pub worktrees: PathBuf,
    pub config: Option<PathBuf>,
    pub tested: Option<String>,
    pub fork: Option<String>,
    pub json: bool,
}

/// Print the report; whether the version is ready.
pub fn run(ask: &Ask) -> Result<bool, String> {
    let config = match &ask.config {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read the config {}: {e}", path.display()))?;
            Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => Config::default(),
    };
    let git = GitCli::new(&ask.repo);
    let clone = git.main_clone().map_err(|e| format!("no clone at {}: {e}", ask.repo.display()))?;
    let home = crate::commands::assemble::rhizoid_home(&ask.worktrees, &clone)?;
    let out = crate::commands::assemble::real_as_far_as_it_exists(&ask.out)?;
    let report = send_back(
        &git,
        &Programs,
        &CoreAsk {
            branch: &ask.branch,
            onto: &ask.onto,
            out: &out,
            home: &home,
            config: &config.send,
            tested: ask.tested.as_deref(),
            fork: ask.fork.as_deref(),
        },
    )
    .map_err(|e| e.to_string())?;
    crate::commands::status::write_report(
        &mut std::io::stdout().lock(),
        &render(&report, ask.json),
    )?;
    Ok(report.ok())
}

/// The report as `key=value` lines, or as JSON.
pub fn render(report: &Report, json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(report).expect("a report serialises") + "\n";
    }
    report.key_values().into_iter().map(|(k, v)| format!("{k}={v}\n")).collect()
}
