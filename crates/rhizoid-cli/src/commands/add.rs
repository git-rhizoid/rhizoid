//! `rhizoid add <owner/repo>` - fork a new module into the configured org.
//!
//! The positional `source` must come first, before any flags (a deliberate
//! v1 simplification - see the commands module doc comment). Everything
//! after it is argenv's to resolve.

use argenv::*;

pub struct Model;
impl Model {
    pub const ORG: Input<String> = Input {
        key: "org",
        ty: Type::String,
        env: Some(Env::new("RHIZOID_ORG")),
        arg: Some(Arg { value_name: "ORG", ..Arg::long("org") }),
        summary: "Override the manifest's default org for this module only",
        example: "--org my-other-org",
        ..Input::EMPTY
    };

    pub const AS_NAME: Input<String> = Input {
        key: "as_name",
        ty: Type::String,
        arg: Some(Arg { value_name: "NAME", ..Arg::long("as") }),
        summary: "The module's name in the manifest (defaults to the upstream repo's own name)",
        example: "--as upstream-thing",
        ..Input::EMPTY
    };

    pub const TRACKED_REF: Input<String> = Input {
        key: "tracked_ref",
        ty: Type::String,
        arg: Some(Arg { value_name: "REF", ..Arg::long("tracked-ref") }),
        summary: "The branch or tag to track (defaults to upstream's own default branch)",
        example: "--tracked-ref main",
        ..Input::EMPTY
    };

    pub fn records() -> Vec<Record> {
        vec![Model::ORG.to_record(), Model::AS_NAME.to_record(), Model::TRACKED_REF.to_record()]
    }

    pub fn problems() -> Vec<String> {
        let mut v = Vec::new();
        v.extend(Model::ORG.check());
        v.extend(Model::AS_NAME.check());
        v.extend(Model::TRACKED_REF.check());
        v
    }
}

pub fn run(source: &str, _resolved: &Resolution) -> Result<(), String> {
    Err(format!(
        "rhizoid add {source}: not implemented yet - forking needs the GitHubPort adapter, \
         which is out of scope for the boundary layer (see rhizoid-core::ports)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_satisfies_argenvs_own_rules() {
        assert!(Model::problems().is_empty(), "{:?}", Model::problems());
    }

    #[test]
    fn as_name_flag_resolves() {
        let args = vec!["--as".to_string(), "renamed".to_string()];
        let resolved =
            Invocation { args: &args, env: &std::collections::BTreeMap::<String, String>::new() }
                .resolve(&Model::records());
        assert_eq!(Model::AS_NAME.get_from(&resolved), Some("renamed".to_string()));
    }

    #[test]
    fn unknown_flag_is_a_lint_finding() {
        let args = vec!["--not-a-real-flag".to_string(), "x".to_string()];
        let invocation =
            Invocation { args: &args, env: &std::collections::BTreeMap::<String, String>::new() };
        let findings = lint(&Model::records(), &invocation);
        assert!(!findings.is_empty(), "expected --not-a-real-flag to be flagged");
    }
}
