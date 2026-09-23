//! `rhizoid remove <module>` - drop a module from the manifest.
//!
//! Never touches the actual fork - removing a module from the manifest just
//! stops Rhizoid tracking it. Needs no GitHubPort adapter, so unlike most of
//! its siblings this one is fully implemented already.

use argenv::*;
use rhizoid_core::Manifest;

pub struct Model;
impl Model {
    pub fn records() -> Vec<Record> {
        vec![]
    }

    pub fn problems() -> Vec<String> {
        vec![]
    }
}

pub fn remove_from(manifest: &mut Manifest, name: &str) -> Result<(), String> {
    let before = manifest.modules.len();
    manifest.modules.retain(|m| m.name != name);
    if manifest.modules.len() == before {
        return Err(format!("no module named `{name}` in the manifest"));
    }
    Ok(())
}

pub fn run(dir: &std::path::Path, name: &str) -> Result<(), String> {
    let mut manifest = crate::manifest_io::load(dir).map_err(|e| e.to_string())?;
    remove_from(&mut manifest, name)?;
    crate::manifest_io::save(dir, &manifest).map_err(|e| e.to_string())?;
    println!("removed {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhizoid_core::{Defaults, ModuleEntry, ModuleOrigin};

    fn manifest_with_one() -> Manifest {
        Manifest {
            defaults: Defaults::default(),
            modules: vec![ModuleEntry {
                name: "example".to_string(),
                source: "octocat/example".to_string(),
                org: None,
                tracked_ref: None,
                origin: ModuleOrigin::Created,
            }],
        }
    }

    #[test]
    fn model_satisfies_argenvs_own_rules() {
        assert!(Model::problems().is_empty());
    }

    #[test]
    fn removes_the_named_module() {
        let mut m = manifest_with_one();
        remove_from(&mut m, "example").unwrap();
        assert!(m.modules.is_empty());
    }

    #[test]
    fn unknown_module_name_is_an_error_and_changes_nothing() {
        let mut m = manifest_with_one();
        let result = remove_from(&mut m, "does-not-exist");
        assert!(result.is_err());
        assert_eq!(m.modules.len(), 1, "manifest must be unchanged on error");
    }
}
