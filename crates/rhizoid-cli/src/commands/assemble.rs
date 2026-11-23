//! `rhizoid assemble LIST` - one named list of a lab's list file merged onto its base in a fresh worktree of
//! rhizoid's home, and tagged `int/DEVICE/LIST/BASE/N`. It ends 0 when tagged (and equal to `--reproduces`, if
//! asked), 1 when it stopped at a conflict or the tree differs, 2 when it could not run.

use crate::git_cli::GitCli;
use rhizoid_core::assemble::{assemble, Ask as CoreAsk, Lists, Report};
use std::path::{Component, Path, PathBuf};

/// What assemble was asked, with every default already decided.
pub struct Ask {
    /// The list's name in the file.
    pub list: String,
    /// The list file.
    pub lists: PathBuf,
    /// The clone the branches are in.
    pub repo: PathBuf,
    /// The agent worktree home (`AGENT_WORKTREES`, else `~/.local/share/agent-worktrees`).
    pub worktrees: PathBuf,
    /// The worktree folder asked for, if any.
    pub worktree: Option<PathBuf>,
    pub fetch: bool,
    pub reproduces: Option<String>,
    pub json: bool,
}

/// The agent worktree home when `AGENT_WORKTREES` is not set: `~/.local/share/agent-worktrees`.
pub fn default_worktrees(home: Option<&str>) -> Result<PathBuf, String> {
    match home {
        Some(h) if !h.is_empty() => Ok(Path::new(h).join(".local/share/agent-worktrees")),
        _ => Err("no AGENT_WORKTREES and no HOME to find ~/.local/share/agent-worktrees from"
            .to_string()),
    }
}

/// Print the report; whether it did what was asked.
pub fn run(ask: &Ask) -> Result<bool, String> {
    let text = std::fs::read_to_string(&ask.lists)
        .map_err(|e| format!("cannot read the list file {}: {e}", ask.lists.display()))?;
    let lists = Lists::parse(&text).map_err(|e| format!("{}: {e}", ask.lists.display()))?;
    let list = lists.get(&ask.list)?;
    let git = GitCli::new(&ask.repo);
    let clone = git.main_clone().map_err(|e| format!("no clone at {}: {e}", ask.repo.display()))?;
    let home = rhizoid_home(&ask.worktrees, &clone)?;
    let worktree = ask.worktree.as_deref().map(real_as_far_as_it_exists).transpose()?;
    let report = assemble(
        &git,
        &CoreAsk {
            name: lists.name.as_deref(),
            tag: &lists.tag,
            list_name: &ask.list,
            list,
            home: &home,
            worktree: worktree.as_deref(),
            fetch: ask.fetch,
            reproduces: ask.reproduces.as_deref(),
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

/// rhizoid's worktree home for a clone, `WORKTREES/CLONE/rhizoid` (named after the main clone's folder), as
/// real a path as exists yet; git makes the folders when it adds the first worktree.
pub fn rhizoid_home(worktrees: &Path, clone: &Path) -> Result<PathBuf, String> {
    let name =
        clone.file_name().ok_or_else(|| format!("{} has no folder name", clone.display()))?;
    real_as_far_as_it_exists(&worktrees.join(name).join("rhizoid"))
}

/// `wanted` (relative to the current folder) with its longest existing part resolved to a real path, so a
/// symbolic link cannot carry it outside the home; `.` and `..` in what does not exist yet are kept, and refused.
pub fn real_as_far_as_it_exists(wanted: &Path) -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current folder: {e}"))?;
    let full = cwd.join(wanted);
    let mut existing = full.clone();
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.file_name(), existing.parent()) {
            (Some(n), Some(p)) => {
                rest.push(n.to_os_string());
                existing = p.to_path_buf();
            }
            _ => break,
        }
    }
    let has_dots = full.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir));
    if has_dots {
        return Ok(full);
    }
    let mut real =
        std::fs::canonicalize(&existing).map_err(|e| format!("{}: {e}", existing.display()))?;
    for n in rest.into_iter().rev() {
        real.push(n);
    }
    Ok(real)
}

/// `rhizoid assemble --done TAG`: unlock and remove the worktree assemble locked for TAG; print what went.
pub fn done(repo: &Path, tag: &str, force: bool) -> Result<(), String> {
    let path =
        rhizoid_core::assemble::done(&GitCli::new(repo), tag, force).map_err(|e| e.to_string())?;
    crate::commands::status::write_report(
        &mut std::io::stdout().lock(),
        &format!("done={tag}\nremoved={path}\n"),
    )
}
