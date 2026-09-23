//! `rhizoid update [module]` - pull upstream into a module's tracking branch
//! (GitHub's own `merge-upstream`). No module name updates every module.

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
            "rhizoid update {name}: not implemented yet - needs the GitHubPort adapter's \
             merge_upstream"
        )),
        None => Err("rhizoid update: not implemented yet - needs the GitHubPort adapter's \
             merge_upstream"
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
