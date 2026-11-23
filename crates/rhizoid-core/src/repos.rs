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

/// What a reference's local tag is in its clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefRead {
    /// The tag is there and peels to this commit.
    At(String),
    /// No such tag.
    Missing,
    /// It was missing, and `--fetch` made it at the reference's commit.
    Made,
    /// It was missing, and `--fetch` could not make it: why.
    NotMade(String),
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

    /// Where `text` appears, as a fixed string, in the tracked files of the clone of `repo` at `rev`: each
    /// file, line (1-based) and the line's text.
    fn grep(
        &self,
        repo: &RepoName,
        rev: &str,
        text: &str,
    ) -> Result<Vec<(String, u64, String)>, RepoError>;

    /// Which of `paths` the clone of `repo` marks at `rev` with the git attribute `attr` (set, or given a value
    /// other than `false`), as its own .gitattributes say.
    fn marked(
        &self,
        repo: &RepoName,
        rev: &str,
        attr: &str,
        paths: &[String],
    ) -> Result<std::collections::BTreeSet<String>, RepoError>;

    /// A reference's local tag in its clone: the commit it peels to, or that it is missing. With `--fetch`, a
    /// missing tag is made from the reference's url and ref when what they give is its commit; an existing tag is
    /// never moved.
    fn reference(&self, r: &crate::graph::Reference) -> Result<RefRead, RepoError>;

    /// What reading found about the clones themselves, beside the edges: a tag `origin` moved that the clone
    /// holds as it was (`moved-by-hand`, `tag-origin`). Ask it after the reads; none by default.
    fn fetch_findings(&self) -> Vec<crate::Finding> {
        Vec::new()
    }
}
