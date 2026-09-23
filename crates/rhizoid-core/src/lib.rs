//! Manifest types, the per-fork metadata type, and the outbound ports for
//! Rhizoid - no CLI, no argv. See `crates/rhizoid-cli` for the boundary that
//! actually talks to a person or a shell.

pub mod manifest;
pub mod module;
pub mod ports;

pub use manifest::{Defaults, Manifest, ModuleEntry, ModuleOrigin};
pub use module::ModuleMetadata;
pub use ports::{GitHubPort, GitPort, MergeOutcome, PortError};
