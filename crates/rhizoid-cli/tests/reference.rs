//! A `[[reference]]` edge against real repositories in a temporary folder: an upstream with a build tag, and a clone
//! (with its own origin) that keeps the reference under a local tag.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgSign=false", "-c", "tag.gpgSign=false"])
        .args(args)
        .envs([
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@example.com"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@example.com"),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct World {
    root: PathBuf,
    clone: PathBuf,
    graph: PathBuf,
    build: String,
    later: String,
}

fn world() -> World {
    let root = std::env::temp_dir().join(format!(
        "rhizoid-reference-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let up = root.join("upstream");
    std::fs::create_dir_all(&up).unwrap();
    git(&up, &["init", "-q", "-b", "main"]);
    git(&up, &["commit", "-q", "--allow-empty", "-m", "the build"]);
    git(&up, &["tag", "-a", "-m", "build 1", "build-1"]);
    let build = git(&up, &["rev-parse", "build-1^{commit}"]);
    git(&up, &["commit", "-q", "--allow-empty", "-m", "later"]);
    let later = git(&up, &["rev-parse", "HEAD"]);
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["commit", "-q", "--allow-empty", "-m", "ours"]);
    let bare = root.join("bare/kernel.git");
    git(&root, &["clone", "-q", "--bare", seed.to_str().unwrap(), bare.to_str().unwrap()]);
    let clone = root.join("acme/kernel");
    git(&root, &["clone", "-q", bare.to_str().unwrap(), clone.to_str().unwrap()]);
    let graph = root.join("graph.toml");
    let edge = |name: &str, r: &str, commit: &str| {
        format!(
            "[[reference]]\nrepo = \"acme/kernel\"\nname = \"{name}\"\nurl = \"{}\"\nref = \"{r}\"\ncommit = \"{commit}\"\ntag = \"reference/{name}/1\"\nwhy = \"as it was at the build\"\n",
            up.display()
        )
    };
    std::fs::write(
        &graph,
        format!("{}\n{}", edge("vendor", "build-1", &build), edge("moved", "main", &build)),
    )
    .unwrap();
    World { root, clone, graph, build, later }
}

impl World {
    fn status(&self, fetch: bool) -> Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_rhizoid"));
        c.args(["status", "--graph"]).arg(&self.graph).arg("--root").arg(&self.root);
        if fetch {
            c.arg("--fetch");
        }
        c.output().unwrap()
    }
}

fn line<'a>(out: &'a str, tag: &str) -> &'a str {
    out.lines()
        .find(|l| l.contains(&format!("reference: {tag} ")))
        .unwrap_or_else(|| panic!("{tag}: {out}"))
}

#[test]
fn fetch_makes_a_missing_reference_tag_at_its_commit_and_never_moves_one() {
    let w = world();
    let out = w.status(false);
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(line(&said, "reference/vendor/1").starts_with("behind"), "{said}");
    assert!(line(&said, "reference/vendor/1").contains("git fetch --no-tags"), "{said}");

    let said = String::from_utf8_lossy(&w.status(true).stdout).to_string();
    assert!(line(&said, "reference/vendor/1").starts_with("current"), "made: {said}");
    assert!(line(&said, "reference/vendor/1").contains("made now"), "{said}");
    assert_eq!(git(&w.clone, &["rev-parse", "reference/vendor/1^{commit}"]), w.build);
    // a branch that moved on since the build gives another commit: no tag made, and why
    let moved = line(&said, "reference/moved/1");
    assert!(moved.starts_with("behind") && moved.contains("not the recorded"), "{said}");
    assert!(git(&w.clone, &["tag", "-l", "reference/moved/*"]).is_empty());

    let said = String::from_utf8_lossy(&w.status(true).stdout).to_string();
    assert!(
        line(&said, "reference/vendor/1").contains("is vendor at build-1"),
        "read as it is: {said}"
    );

    // moved by hand: said, and --fetch leaves it where it is
    git(&w.clone, &["tag", "-f", "reference/vendor/1", "HEAD"]);
    let here = git(&w.clone, &["rev-parse", "reference/vendor/1^{commit}"]);
    let said = String::from_utf8_lossy(&w.status(true).stdout).to_string();
    assert!(line(&said, "reference/vendor/1").starts_with("moved-by-hand"), "{said}");
    assert_eq!(git(&w.clone, &["rev-parse", "reference/vendor/1^{commit}"]), here, "never moved");
    assert_ne!(here, w.later);
}
