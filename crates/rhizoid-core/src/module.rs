//! `.rhizoid/module.toml` - the metadata file added to a fork itself.
//!
//! This is the additive part of the additive-file-structure design: a fork
//! gets exactly one new file at a new path, never an edit to anything
//! upstream already had, so a future `update` (a fast-forward merge from
//! upstream) never conflicts with it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::manifest::ModuleOrigin;

/// The state recorded inside a managed fork, at `.rhizoid/module.toml`.
///
/// Unlike [`crate::manifest::ModuleEntry`] (the workspace's record of
/// *intent*), this is the fork's own record of *fact*: exactly where it came
/// from and what it was last synced to, written by the fork itself, not by
/// whatever happens to be consuming it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleMetadata {
    /// The upstream repository this fork was created from, as `owner/repo`.
    pub origin: String,

    /// The branch or tag tracking upstream.
    pub tracked_ref: String,

    /// The upstream commit the tracking branch was last synced to.
    pub last_synced_commit: String,

    /// Whether this fork was created by `add` or adopted by `import`.
    pub added_as: ModuleOrigin,

    /// The date (`YYYY-MM-DD`) this file was first written.
    pub attached_on: String,
}

/// This file's own JSON Schema, for the same editor-tooling reasons as the
/// top-level manifest's.
pub fn json_schema() -> schemars::schema::RootSchema {
    schemars::schema_for!(ModuleMetadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ModuleMetadata {
        ModuleMetadata {
            origin: "octocat/example".to_string(),
            tracked_ref: "main".to_string(),
            last_synced_commit: "abc123".to_string(),
            added_as: ModuleOrigin::Created,
            attached_on: "2026-09-22".to_string(),
        }
    }

    #[test]
    fn round_trips_through_toml() {
        let m = sample();
        let text = toml::to_string_pretty(&m).expect("serialises");
        let back: ModuleMetadata = toml::from_str(&text).expect("parses");
        assert_eq!(m, back);
    }

    #[test]
    fn every_field_is_required_no_silent_defaults() {
        // A module's own fact-record should never silently fill in a missing
        // field - if something is missing, that is real drift, and status
        // should report it as such rather than paper over it.
        let text = r#"
            origin = "octocat/example"
            tracked_ref = "main"
            last_synced_commit = "abc123"
            added_as = "created"
        "#; // attached_on is missing
        assert!(toml::from_str::<ModuleMetadata>(text).is_err());
    }
}
