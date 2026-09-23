//! Reading and writing `rhizoid.toml` in the current directory.

use rhizoid_core::Manifest;
use std::path::Path;

pub const MANIFEST_PATH: &str = "rhizoid.toml";

/// The `#:schema` directive `init` writes at the top of a fresh manifest, so
/// VS Code (Even Better TOML), Sublime and Neovim's Taplo-backed plugins give
/// autocomplete, hover docs and validation with no per-project setup - see
/// the design log's `schema-driven-editor-support` entry for why.
///
/// Points at a fixed, unversioned path for now (this repo's own schema.json,
/// tracking whatever is on `main`) - see the manifest-file-format concept for
/// why there is deliberately no version-pinned URL scheme yet.
pub const SCHEMA_DIRECTIVE: &str =
    "#:schema https://raw.githubusercontent.com/git-rhizoid/rhizoid/main/schema.json";

#[derive(Debug)]
pub enum LoadError {
    NotFound,
    Io(String),
    Parse(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NotFound => {
                write!(f, "no {MANIFEST_PATH} in the current directory - run `rhizoid init` first")
            }
            LoadError::Io(e) => write!(f, "cannot read {MANIFEST_PATH}: {e}"),
            LoadError::Parse(e) => write!(f, "{MANIFEST_PATH} is not valid: {e}"),
        }
    }
}

pub fn load(dir: &Path) -> Result<Manifest, LoadError> {
    let path = dir.join(MANIFEST_PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            LoadError::NotFound
        } else {
            LoadError::Io(e.to_string())
        }
    })?;
    toml::from_str(&text).map_err(|e| LoadError::Parse(e.to_string()))
}

pub fn save(dir: &Path, manifest: &Manifest) -> std::io::Result<()> {
    let body = toml::to_string_pretty(manifest).expect("Manifest always serialises");
    let text = format!("{SCHEMA_DIRECTIVE}\n\n{body}");
    std::fs::write(dir.join(MANIFEST_PATH), text)
}

pub fn exists(dir: &Path) -> bool {
    dir.join(MANIFEST_PATH).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhizoid_core::{Defaults, ModuleEntry, ModuleOrigin};

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir();
        let m = Manifest {
            defaults: Defaults { org: "git-rhizoid".to_string() },
            modules: vec![ModuleEntry {
                name: "example".to_string(),
                source: "octocat/example".to_string(),
                org: None,
                tracked_ref: None,
                origin: ModuleOrigin::Created,
            }],
        };
        save(&dir, &m).unwrap();
        let back = load(&dir).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn save_writes_the_schema_directive_as_the_first_line() {
        let dir = tempdir();
        let m = Manifest { defaults: Defaults::default(), modules: vec![] };
        save(&dir, &m).unwrap();
        let text = std::fs::read_to_string(dir.join(MANIFEST_PATH)).unwrap();
        assert_eq!(text.lines().next(), Some(SCHEMA_DIRECTIVE));
    }

    #[test]
    fn missing_manifest_is_a_distinct_error() {
        let dir = tempdir();
        assert!(matches!(load(&dir), Err(LoadError::NotFound)));
        assert!(!exists(&dir));
    }

    #[test]
    fn unparsable_manifest_is_a_distinct_error() {
        let dir = tempdir();
        std::fs::write(dir.join(MANIFEST_PATH), "not = [valid toml").unwrap();
        assert!(matches!(load(&dir), Err(LoadError::Parse(_))));
    }

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rhizoid-test-{}", uuid_ish()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid_ish() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    }
}
