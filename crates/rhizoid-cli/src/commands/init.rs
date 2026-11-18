//! `rhizoid init` - write a fresh `rhizoid.toml` in the current directory.

use rhizoid_core::{Defaults, Manifest};
use std::path::Path;

/// Effect boundary kept thin on purpose: this function only decides
/// `Manifest` in / `Result` out, so it is testable with no filesystem at all.
pub fn build_manifest(org: Option<String>) -> Manifest {
    Manifest {
        defaults: Defaults {
            org: org.filter(|s| !s.is_empty()).unwrap_or_else(|| Defaults::default().org),
        },
        modules: vec![],
    }
}

pub fn run(dir: &Path, org: Option<String>, force: bool) -> Result<(), String> {
    if crate::manifest_io::exists(dir) && !force {
        return Err(format!(
            "{} already exists - pass --force to overwrite",
            crate::manifest_io::MANIFEST_PATH
        ));
    }
    let manifest = build_manifest(org);
    crate::manifest_io::save(dir, &manifest).map_err(|e| e.to_string())?;
    println!("wrote {}", crate::manifest_io::MANIFEST_PATH);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn an_empty_org_is_no_org() {
        let m = build_manifest(Some(String::new()));
        assert_eq!(m.defaults.org, Defaults::default().org);
    }
}
