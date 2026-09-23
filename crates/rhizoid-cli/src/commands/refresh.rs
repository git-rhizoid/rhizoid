//! `rhizoid refresh [module]` - reconcile the manifest to match observed
//! reality, without changing any git state (mirrors `terraform apply
//! -refresh-only` - see the design log's `drift-detection` entry).

use argenv::*;

pub struct Model;
impl Model {
    pub fn records() -> Vec<Record> {
        vec![]
    }

    pub fn problems() -> Vec<String> {
        vec![]
    }
}

pub fn run(module: Option<&str>, _resolved: &Resolution) -> Result<(), String> {
    match module {
        Some(name) => Err(format!(
            "rhizoid refresh {name}: not implemented yet - needs the GitHubPort adapter \
             to observe the fork's real state"
        )),
        None => Err("rhizoid refresh: not implemented yet - needs the GitHubPort adapter \
             to observe each fork's real state"
            .to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_satisfies_argenvs_own_rules() {
        assert!(Model::problems().is_empty());
    }
}
