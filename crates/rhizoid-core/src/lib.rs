//! Manifest types, the graph and its status, the per-fork metadata type, and the outbound ports for
//! Rhizoid - no CLI, no argv. See `crates/rhizoid-cli` for the boundary that
//! actually talks to a person or a shell.

pub mod assemble;
pub mod finding;
pub mod graph;
pub mod integration;
pub mod manifest;
pub mod module;
pub mod pattern;
pub mod ports;
pub mod repos;
pub mod sendback;
pub mod status;
pub mod text;

pub use finding::{drift_count, summary, Check, Finding, Verdict};
pub use graph::{Graph, RepoName};
pub use integration::{integration_status, IntegrationReads, Worktree};
pub use manifest::{Defaults, Manifest, ModuleEntry, ModuleOrigin};
pub use module::ModuleMetadata;
pub use ports::{
    BranchSource, CommandPort, FormatPatch, GitHubPort, GitPort, MergeOutcome, MergeResult,
    PortError, RebaseResult,
};
pub use repos::{Level, RepoError, Repos};
pub use status::status;
