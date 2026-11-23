//! Outbound ports: what the core needs from git and GitHub, as traits.
//!
//! Hexagonal-style on purpose (see the canon this project keeps citing
//! itself against): the core depends on these interfaces, never on `git`/`gh`
//! directly, so the real adapter (shelling out to `gh`, per
//! `auth-via-gh`) can be swapped or, for now, simply not exist yet without
//! blocking everything that sits above it.
//!
//! No implementation lives in this crate: the adapters are in `rhizoid-cli`
//! (`GitCli` for [`GitPort`]; the GitHub one is not built yet - see the design
//! log's `design-boundary-layer` entry).

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

/// What Rhizoid needs from one local clone, as git's own work: rhizoid drives the git CLI for it
/// (decision `2026-11-23-standard-tools-drive-cut-or-write`), so worktrees and the rerere memory are
/// exactly the ones a person's git sees. The real adapter is `rhizoid-cli`'s `GitCli`.
pub trait GitPort {
    /// Fetch every remote, pruning the branches they no longer have (`git fetch --all --prune`).
    fn fetch_prune(&self) -> Result<(), PortError>;

    /// The commit `rev` names (a tag, a branch, a sha), or `None` when the clone has none.
    fn commit(&self, rev: &str) -> Result<Option<String>, PortError>;

    /// The commit branch `name` points at: the local branch, else `origin`'s; `None` when neither exists.
    fn branch(&self, name: &str) -> Result<Option<String>, PortError>;

    /// The tags whose names start with `prefix`.
    fn tags(&self, prefix: &str) -> Result<Vec<String>, PortError>;

    /// Turn on the clone's shared rerere memory (`rerere.enabled` in its config), so every hand on the clone -
    /// rhizoid, a person resolving a conflict - records and replays the same resolutions.
    fn share_rerere(&self) -> Result<(), PortError>;

    /// A fresh worktree at `path`, detached at `commit` (`git worktree add --detach`).
    fn add_worktree(&self, path: &std::path::Path, commit: &str) -> Result<(), PortError>;

    /// Lock the worktree at `path` so `git worktree prune` keeps it (`git worktree lock --reason`).
    fn lock_worktree(&self, path: &std::path::Path, reason: &str) -> Result<(), PortError>;

    /// Merge `commit` into the worktree at `path` as a merge commit with `message`, rerere on.
    fn merge(
        &self,
        path: &std::path::Path,
        commit: &str,
        message: &str,
    ) -> Result<MergeResult, PortError>;

    /// The paths `commit` changed since it left `base` (`git diff --name-only base...commit`).
    fn changed_paths(&self, base: &str, commit: &str) -> Result<Vec<String>, PortError>;

    /// The commit the worktree at `path` is at.
    fn head(&self, path: &std::path::Path) -> Result<String, PortError>;

    /// An annotated tag `name` on `commit` with `message`; the tag object's sha.
    fn tag(&self, name: &str, commit: &str, message: &str) -> Result<String, PortError>;

    /// The paths whose content differs between the trees of `a` and `b`; empty when the trees are equal.
    fn tree_diff(&self, a: &str, b: &str) -> Result<Vec<String>, PortError>;
}

/// How one merge into a worktree ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeResult {
    /// The merge commit was made; `by_rerere` when a conflict was resolved from rerere's memory.
    Merged {
        /// A conflict came up and every path of it was resolved from a recorded resolution.
        by_rerere: bool,
    },
    /// The commit was already in the worktree's history: nothing was merged.
    AlreadyContained,
    /// The merge stopped with these paths still in conflict; the worktree is left mid-merge.
    Conflict {
        /// The unmerged paths.
        paths: Vec<String>,
    },
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
