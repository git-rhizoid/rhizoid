//! `rhizoid init` - write a fresh `rhizoid.toml` in the current directory.

use argenv::*;
use rhizoid_core::{Defaults, Manifest};
use std::path::Path;

pub struct Model;
impl Model {
    pub const ORG: Input<String> = Input {
        key: "org",
        ty: Type::String,
        default: Some(String::new()),
        env: Some(Env::new("RHIZOID_ORG")),
        arg: Some(Arg { value_name: "ORG", ..Arg::long("org") }),
        summary: "The GitHub org or user new forks are created under by default",
        example: "--org git-rhizoid",
        ..Input::EMPTY
    };

    pub const FORCE: Input<bool> = Input {
        key: "force",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg::long("force")),
        summary: "Overwrite an existing rhizoid.toml",
        example: "--force",
        ..Input::EMPTY
    };

    pub fn records() -> Vec<Record> {
        vec![Model::ORG.to_record(), Model::FORCE.to_record()]
    }

    pub fn problems() -> Vec<String> {
        let mut v = Vec::new();
        v.extend(Model::ORG.check());
        v.extend(Model::FORCE.check());
        v
    }
}

/// Effect boundary kept thin on purpose: this function only decides
/// `Manifest` in / `Result` out, so it is testable with no filesystem at all
/// - see `tests::init_writes_the_default_org_when_none_given` below.
pub fn build_manifest(org: Option<String>) -> Manifest {
    Manifest {
        defaults: Defaults {
            org: org.filter(|s| !s.is_empty()).unwrap_or_else(|| Defaults::default().org),
        },
        modules: vec![],
    }
}

pub fn run(dir: &Path, resolved: &Resolution) -> Result<(), String> {
    if crate::manifest_io::exists(dir)
        && !Model::FORCE.get_from_or_default(resolved).unwrap_or(false)
    {
        return Err(format!(
            "{} already exists - pass --force to overwrite",
            crate::manifest_io::MANIFEST_PATH
        ));
    }
    let org = Model::ORG.get_from(resolved).filter(|s| !s.is_empty());
    let manifest = build_manifest(org);
    crate::manifest_io::save(dir, &manifest).map_err(|e| e.to_string())?;
    println!("wrote {}", crate::manifest_io::MANIFEST_PATH);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_satisfies_argenvs_own_rules() {
        assert!(Model::problems().is_empty(), "{:?}", Model::problems());
    }

    #[test]
    fn init_writes_the_default_org_when_none_given() {
        let m = build_manifest(None);
        assert_eq!(m.defaults.org, Defaults::default().org);
        assert!(m.modules.is_empty());
    }

    #[test]
    fn init_writes_the_given_org() {
        let m = build_manifest(Some("my-org".to_string()));
        assert_eq!(m.defaults.org, "my-org");
    }

    #[test]
    fn org_flag_beats_env() {
        use std::collections::BTreeMap;
        let env: BTreeMap<String, String> =
            [("RHIZOID_ORG".to_string(), "from-env".to_string())].into();
        let args = vec!["--org".to_string(), "from-arg".to_string()];
        let resolved = Invocation { args: &args, env: &env }.resolve(&Model::records());
        assert_eq!(Model::ORG.get_from(&resolved), Some("from-arg".to_string()));
    }

    #[test]
    fn env_is_used_when_no_flag_given() {
        use std::collections::BTreeMap;
        let env: BTreeMap<String, String> =
            [("RHIZOID_ORG".to_string(), "from-env".to_string())].into();
        let args: Vec<String> = vec![];
        let resolved = Invocation { args: &args, env: &env }.resolve(&Model::records());
        assert_eq!(Model::ORG.get_from(&resolved), Some("from-env".to_string()));
    }
}
