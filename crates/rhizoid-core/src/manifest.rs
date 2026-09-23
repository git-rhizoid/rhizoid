//! The manifest: `rhizoid.toml`, the human-edited record of intent.
//!
//! Deliberately not a lockfile. Each managed fork carries its own exact
//! resolved state in its own `.rhizoid/module.toml` (see [`crate::module`]) -
//! this file records what a workspace *wants* to track, not the exact commit
//! each one is currently pinned to. `status`/`refresh` read the fork's own
//! file for that, not a central copy that could drift from it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The whole manifest: workspace-wide defaults, plus one entry per tracked
/// module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    /// Workspace-wide defaults every module falls back to unless it overrides
    /// them.
    #[serde(default)]
    pub defaults: Defaults,

    /// One entry per module this workspace tracks. A TOML array of tables:
    /// each `[[module]]` block below is one entry.
    #[serde(default, rename = "module")]
    pub modules: Vec<ModuleEntry>,
}

/// Workspace-wide defaults. Any module may override any of these for itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Defaults {
    /// The GitHub org (or user account) new forks are created under, unless a
    /// module names its own `org`.
    pub org: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults { org: "git-rhizoid".to_string() }
    }
}

/// One tracked module: an entry in the manifest's `[[module]]` array.
///
/// This is the *intent* record (what should exist, and where). The exact,
/// currently-observed state of the fork itself lives in that fork's own
/// `.rhizoid/module.toml`, not here - see the crate-level docs for why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleEntry {
    /// This module's name within the manifest - how commands refer to it
    /// (`rhizoid update <name>`). Defaults to the upstream repo's own name if
    /// not given.
    pub name: String,

    /// The upstream repository, as `owner/repo`.
    pub source: String,

    /// Which GitHub org or user this module's fork lives under. Overrides
    /// `defaults.org` for this module only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,

    /// The branch or tag on the fork that tracks upstream (fast-forwarded via
    /// `update`, never hand-edited). Defaults to upstream's own default
    /// branch if not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracked_ref: Option<String>,

    /// Whether this module was created by `add` (a new fork) or adopted by
    /// `import` (an existing one, not created by Rhizoid).
    pub origin: ModuleOrigin,
}

/// How a module's fork came to be managed by Rhizoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleOrigin {
    /// Forked by `rhizoid add`.
    Created,
    /// Adopted by `rhizoid import`, from a fork that already existed.
    Imported,
}

/// The manifest's own JSON Schema, for editor tooling (`#:schema`, hover
/// docs, autocomplete) - see the design log's `schema-driven-editor-support`.
/// Derived from these same types via `schemars`, so it cannot drift from what
/// this crate actually accepts: there is no second, hand-maintained copy.
pub fn json_schema() -> schemars::schema::RootSchema {
    schemars::schema_for!(Manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        Manifest {
            defaults: Defaults { org: "git-rhizoid".to_string() },
            modules: vec![ModuleEntry {
                name: "example".to_string(),
                source: "octocat/example".to_string(),
                org: None,
                tracked_ref: Some("main".to_string()),
                origin: ModuleOrigin::Created,
            }],
        }
    }

    #[test]
    fn round_trips_through_toml() {
        let m = sample();
        let text = toml::to_string_pretty(&m).expect("serialises");
        let back: Manifest = toml::from_str(&text).expect("parses");
        assert_eq!(m, back);
    }

    #[test]
    fn defaults_org_has_a_sensible_default() {
        assert_eq!(Defaults::default().org, "git-rhizoid");
    }

    #[test]
    fn an_empty_manifest_parses() {
        let m: Manifest = toml::from_str("").expect("parses");
        assert_eq!(m.modules.len(), 0);
        assert_eq!(m.defaults.org, "git-rhizoid");
    }

    #[test]
    fn module_without_org_or_tracked_ref_parses() {
        let text = r#"
            [[module]]
            name = "example"
            source = "octocat/example"
            origin = "created"
        "#;
        let m: Manifest = toml::from_str(text).expect("parses");
        assert_eq!(m.modules[0].org, None);
        assert_eq!(m.modules[0].tracked_ref, None);
    }

    #[test]
    fn schema_generates_and_is_stable_across_calls() {
        let a = serde_json::to_string(&json_schema()).unwrap();
        let b = serde_json::to_string(&json_schema()).unwrap();
        assert_eq!(a, b, "schema generation must be deterministic");
        assert!(a.contains("\"tracked_ref\""));
    }

    #[test]
    fn imported_module_serialises_as_kebab_case() {
        let mut m = sample();
        m.modules[0].origin = ModuleOrigin::Imported;
        let text = toml::to_string(&m).unwrap();
        assert!(text.contains("origin = \"imported\""));
    }
}
