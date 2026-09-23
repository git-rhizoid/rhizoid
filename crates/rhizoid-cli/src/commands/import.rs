//! `rhizoid import <owner/repo>` - adopt an existing fork without creating one.
//!
//! Deliberately a separate command from `add`, not a flag on it - see the
//! design log's `adopt-vs-create` entry for why.

use argenv::*;

pub struct Model;
impl Model {
    pub const AS_NAME: Input<String> = Input {
        key: "as_name",
        ty: Type::String,
        arg: Some(Arg { value_name: "NAME", ..Arg::long("as") }),
        summary: "The module's name in the manifest (defaults to the fork repo's own name)",
        example: "--as upstream-thing",
        ..Input::EMPTY
    };

    pub const TRACKED_REF: Input<String> = Input {
        key: "tracked_ref",
        ty: Type::String,
        arg: Some(Arg { value_name: "REF", ..Arg::long("tracked-ref") }),
        summary: "The branch or tag on the fork that tracks upstream",
        example: "--tracked-ref main",
        ..Input::EMPTY
    };

    pub fn records() -> Vec<Record> {
        vec![Model::AS_NAME.to_record(), Model::TRACKED_REF.to_record()]
    }

    pub fn problems() -> Vec<String> {
        let mut v = Vec::new();
        v.extend(Model::AS_NAME.check());
        v.extend(Model::TRACKED_REF.check());
        v
    }
}

pub fn run(fork: &str, _resolved: &Resolution) -> Result<(), String> {
    Err(format!(
        "rhizoid import {fork}: not implemented yet - needs the GitHubPort adapter \
         to confirm the fork exists and is reachable"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_satisfies_argenvs_own_rules() {
        assert!(Model::problems().is_empty(), "{:?}", Model::problems());
    }
}
