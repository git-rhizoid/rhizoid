//! What `status` needs to read from the world: files at a revision of a clone, the tags of a clone, the head of a remote.
//! A port, so the checks are a function of what was read; the real reader shells out to git (rhizoid-cli).

use crate::graph::{Remote, RepoName};

/// Why the world could not be read. Every one stops the report: a verdict over a guess is worse than none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoError {
    /// No clone of this repository is where the graph says.
    NoClone(String),
    /// The clone has no such revision (a tag the source never made, a ref never fetched).
    NoRev {
        /// The repository whose clone was asked.
        repo: String,
        /// The revision it lacks.
        rev: String,
    },
    /// Anything else git or the network said, or a file that is not what its reader needs.
    Failed(String),
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoError::NoClone(p) => write!(f, "no clone at {p}"),
            RepoError::NoRev { repo, rev } => {
                write!(f, "{repo} has no revision {rev} (fetch it, or rhizoid status --fetch)")
            }
            RepoError::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for RepoError {}

/// How a fork's head stands against its upstream's, when the two differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Level {
    /// The fork lacks this many commits of the upstream's.
    Behind(u64),
    /// The fork has this many commits the upstream lacks.
    Ahead(u64),
    /// Each has commits the other lacks.
    Diverged {
        /// What the fork lacks.
        behind: u64,
        /// What the fork has beyond.
        ahead: u64,
    },
    /// The host could not say how they differ, and why.
    Unknown(String),
}

/// Read-only access to the repositories of a graph.
pub trait Repos {
    /// The text of `path` at `rev` in the clone of `repo`; `None` when the revision has no such file.
    fn file(&self, repo: &RepoName, rev: &str, path: &str) -> Result<Option<String>, RepoError>;

    /// Every tag of the clone of `repo`.
    fn tags(&self, repo: &RepoName) -> Result<Vec<String>, RepoError>;

    /// The commit a remote's branch (its HEAD when none) points at.
    fn head(&self, remote: &Remote, branch: Option<&str>) -> Result<String, RepoError>;

    /// How `fork_head` stands against `upstream_head`, two heads that are not the same.
    fn level(&self, fork: &Remote, fork_head: &str, upstream_head: &str) -> Level;
}
