//! `rhizoid follow --source DIR BASE VERSION` - lay release vVERSION of a source into a base, exactly as
//! ideal-knowledge-base's bin/follow.sh does: what it prints, the files it writes and how it ends (0, 3 when the
//! base had changed a generic file, 2 when it could not run).

use crate::git_cli::{GitCli, Programs};
use std::path::{Component, Path, PathBuf};

/// What follow was asked.
pub struct Ask {
    pub source: PathBuf,
    pub base: PathBuf,
    pub version: String,
}

/// Lay the release; the exit code the shell would give.
pub fn run(ask: &Ask) -> Result<u8, String> {
    // the shell's `cd DIR && pwd`: a folder that is not there ends it with 1, and the path stays as named
    let (Some(source), Some(base)) = (logical(&ask.source), logical(&ask.base)) else {
        eprintln!(
            "rhizoid follow: cannot cd to {} or {}",
            ask.source.display(),
            ask.base.display()
        );
        return Ok(1);
    };
    let done = rhizoid_core::follow::follow(
        &GitCli::new(&source),
        &Programs,
        &source,
        &base,
        &ask.version,
    )
    .map_err(|e| e.to_string())?;
    use std::io::Write;
    std::io::stdout().write_all(done.said.as_bytes()).map_err(|e| e.to_string())?;
    std::io::stderr().write_all(done.complained.as_bytes()).map_err(|e| e.to_string())?;
    Ok(done.code as u8)
}

/// A folder's absolute path as `pwd` gives it after `cd`: `.` and `..` worked out by name, links kept as named.
fn logical(dir: &Path) -> Option<PathBuf> {
    let full = std::env::current_dir().ok()?.join(dir);
    let mut out = PathBuf::new();
    for c in full.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out.is_dir().then_some(out)
}
