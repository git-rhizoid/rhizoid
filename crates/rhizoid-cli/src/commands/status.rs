//! `rhizoid status` - report drift between the manifest and real state.
//!
//! What this can honestly report without the GitHubPort adapter: whether the
//! manifest exists and parses, and how many modules it declares. Whether
//! each module's fork actually exists, and whether its tracking branch is
//! ahead/behind, needs the adapter - see `rhizoid-core::ports`.

use argenv::*;
use rhizoid_core::Manifest;

pub struct Model;
impl Model {
    pub const JSON: Input<bool> = Input {
        key: "json",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg::long("json")),
        summary: "Machine-readable output",
        example: "--json",
        ..Input::EMPTY
    };

    pub fn records() -> Vec<Record> {
        vec![Model::JSON.to_record()]
    }

    pub fn problems() -> Vec<String> {
        Model::JSON.check()
    }
}

pub struct Report {
    pub module_count: usize,
    pub module_names: Vec<String>,
}

pub fn build_report(manifest: &Manifest) -> Report {
    Report {
        module_count: manifest.modules.len(),
        module_names: manifest.modules.iter().map(|m| m.name.clone()).collect(),
    }
}

pub fn run(manifest: &Manifest, resolved: &Resolution) {
    let report = build_report(manifest);
    if Model::JSON.get_from_or_default(resolved).unwrap_or(false) {
        println!(
            "{}",
            serde_json::json!({
                "module_count": report.module_count,
                "modules": report.module_names,
                "drift": "unknown - GitHubPort adapter not implemented yet",
            })
        );
    } else {
        println!("{} module(s) declared:", report.module_count);
        for name in &report.module_names {
            println!("  {name}  (drift: unknown - GitHubPort adapter not implemented yet)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhizoid_core::{Defaults, ModuleEntry, ModuleOrigin};

    #[test]
    fn model_satisfies_argenvs_own_rules() {
        assert!(Model::problems().is_empty());
    }

    #[test]
    fn report_counts_and_names_modules() {
        let m = Manifest {
            defaults: Defaults::default(),
            modules: vec![ModuleEntry {
                name: "example".to_string(),
                source: "octocat/example".to_string(),
                org: None,
                tracked_ref: None,
                origin: ModuleOrigin::Created,
            }],
        };
        let report = build_report(&m);
        assert_eq!(report.module_count, 1);
        assert_eq!(report.module_names, vec!["example".to_string()]);
    }

    #[test]
    fn report_on_an_empty_manifest_is_empty_not_an_error() {
        let m = Manifest { defaults: Defaults::default(), modules: vec![] };
        let report = build_report(&m);
        assert_eq!(report.module_count, 0);
    }
}
