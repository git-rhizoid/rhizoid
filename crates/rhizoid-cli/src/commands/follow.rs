//! `rhizoid follow --source DIR BASE VERSION` - lay release vVERSION of a source into a base, exactly as
//! ideal-knowledge-base's bin/follow.sh does: what it prints, the files it writes and how it ends (0, 3 when the
//! base had changed a generic file, 2 when it could not run).

use crate::git_cli::{GitCli, Programs};
use std::path::PathBuf;

/// What follow was asked.
pub struct Ask {
    pub source: PathBuf,
    pub base: PathBuf,
    pub version: String,
}

/// Lay the release; the exit code the shell would give.
pub fn run(ask: &Ask) -> Result<u8, String> {
    let source =
        std::fs::canonicalize(&ask.source).map_err(|e| format!("{}: {e}", ask.source.display()))?;
    let base =
        std::fs::canonicalize(&ask.base).map_err(|e| format!("{}: {e}", ask.base.display()))?;
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
