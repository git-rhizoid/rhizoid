//! Outbound ports: what the core needs from git and GitHub, as traits.
//!
//! Hexagonal-style on purpose (see the canon this project keeps citing
//! itself against): the core depends on these interfaces, never on `git`/`gh`
//! directly, so the real adapter (shelling out to `gh`, per
//! `auth-via-gh`) can be swapped or, for now, simply not exist yet without
//! blocking everything that sits above it.
//!
//! No implementation lives in this crate yet - see the design log's
//! `design-boundary-layer` entry for why that is a deliberate scope cut for
//! this pass, not an oversight.

/// What Rhizoid needs from GitHub itself, independent of local git state.
pub trait GitHubPort {
    /// Fork `source` (`owner/repo`) into `org`. Returns the new fork's
    /// `owner/repo`.
    fn fork(&self, source: &str, org: &str) -> Result<String, PortError>;

    /// Fast-forward `fork`'s `branch` to match its upstream parent (GitHub's
    /// own `merge-upstream` endpoint - see the `wei-pull`/`github-merge-upstream`
    /// sources this design already cites).
    fn merge_upstream(&self, fork: &str, branch: &str) -> Result<MergeOutcome, PortError>;

    /// Whether `full_name` (`owner/repo`) exists and is reachable.
    fn repo_exists(&self, full_name: &str) -> Result<bool, PortError>;
}

/// What Rhizoid needs from local git operations against a module's own
/// `.rhizoid/module.toml`.
pub trait GitPort {
    /// The commit a fork's tracking branch currently points at.
    fn current_commit(&self, fork: &str, branch: &str) -> Result<String, PortError>;
}

/// The result of a `merge_upstream` call: whether anything actually changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The tracking branch was already level with upstream.
    AlreadyUpToDate,
    /// The tracking branch was fast-forwarded to this new commit.
    FastForwarded { new_commit: String },
}

/// A failure from either port. Deliberately coarse for now - this is a
/// boundary the real adapter will need to refine once it exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortError(pub String);

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PortError {}
