//! `rhizoid status` against real git repositories made in a temporary folder: the clones are read with git,
//! the remotes asked with `git ls-remote`, and the process ends with the status the report earns.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-status-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A repository `root/name` on branch main with these files, committed; tagged when a tag is given.
fn repo(root: &Path, name: &str, files: &[(&str, &str)], tag: Option<&str>) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    if !dir.join(".git").exists() {
        git(&dir, &["init", "-q", "-b", "main"]);
    }
    for (path, text) in files {
        std::fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
        std::fs::write(dir.join(path), text).unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "change", "--allow-empty"]);
    if let Some(t) = tag {
        git(&dir, &["tag", t]);
    }
    dir
}

const PART_V1: &str = "# kb:begin\n[tables.one]\n# kb:end\n";
const PART_V2: &str = "# kb:begin\n[tables.one]\n[tables.two]\n# kb:end\n";

/// A world where every edge is current: a source with two releases, a follower at the second, a tool, a fork.
fn world(root: &Path) -> PathBuf {
    repo(root, "acme/kb", &[("tables.toml", PART_V1), ("records/x.sql", "x")], Some("v0.1.0"));
    repo(root, "acme/kb", &[("tables.toml", PART_V2)], Some("v0.2.0"));
    repo(
        root,
        "acme/app",
        &[
            ("kb.toml", "version = \"0.2.0\"\nown = [\"tables.toml\", \"records/x.sql\"]\n"),
            ("tables.toml", PART_V2),
            ("records/x.sql", "x"),
            ("bin/tool.sh", "TOOL=${T:-1.0.0}\n"),
        ],
        None,
    );
    repo(root, "acme/tools", &[("README", "r")], Some("tool-v1.0.0"));
    let upstream = repo(root, "up/stream", &[("a", "a")], None);
    let fork = repo(root, "fork/stream", &[("a", "a")], None);
    // the same commit on both: the fork is the upstream
    git(&fork, &["fetch", "-q", upstream.to_str().unwrap(), "main"]);
    git(&fork, &["reset", "-q", "--hard", "FETCH_HEAD"]);
    let graph = format!(
        r#"
rev = "main"
[[slice]]
source = "acme/kb"
pin = "kb.toml"
marker = "kb"
followers = ["acme/app"]

[[tool]]
name = "tool"
source = "acme/tools"
tag_prefix = "tool-v"
pin = "bin/tool.sh"
line = "TOOL=${{T:-{{version}}}}"
followers = ["acme/app"]

[[fork]]
fork = "{}"
branch = "main"
upstream = "{}"
upstream_branch = "main"
"#,
        fork.display(),
        upstream.display()
    );
    let file = root.join("graph.toml");
    std::fs::write(&file, graph).unwrap();
    file
}

fn status(root: &Path, graph: &Path, extra: &[&str]) -> Output {
    Command::new(bin())
        .arg("status")
        .arg("--root")
        .arg(root)
        .arg("--graph")
        .arg(graph)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn a_world_where_every_edge_is_current_ends_0_and_says_so_line_by_line() {
    let root = tempdir();
    let graph = world(&root);
    let out = status(&root, &graph, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    for check in ["slice-pin", "own-part", "own-behind", "tool-pin", "fork-track"] {
        assert!(
            text.lines().any(|l| l.starts_with("current") && l.contains(check)),
            "{check} in\n{text}"
        );
    }
    assert!(text.contains("0 drift"), "{text}");
}

#[test]
fn a_follower_pinned_behind_ends_1_and_names_the_release_it_lacks() {
    let root = tempdir();
    let graph = world(&root);
    repo(
        &root,
        "acme/app",
        &[("kb.toml", "version = \"0.1.0\"\nown = [\"records/x.sql\"]\n")],
        None,
    );
    let out = status(&root, &graph, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    let line =
        text.lines().find(|l| l.starts_with("behind") && l.contains("slice-pin")).expect(&text);
    assert!(line.contains("0.1.0") && line.contains("0.2.0"), "{line}");
}

#[test]
fn a_fork_that_fell_behind_its_upstream_is_drift() {
    let root = tempdir();
    let graph = world(&root);
    repo(&root, "up/stream", &[("b", "b")], None);
    let out = status(&root, &graph, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(stdout(&out).lines().any(|l| l.starts_with("behind") && l.contains("fork-track")));
}

#[test]
fn a_clone_that_is_not_there_ends_2_and_names_where_it_looked() {
    let root = tempdir();
    let graph = world(&root);
    std::fs::remove_dir_all(root.join("acme/tools")).unwrap();
    let out = status(&root, &graph, &[]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no clone at") && err.contains("acme/tools"), "{err}");
    assert!(stdout(&out).is_empty(), "no partial report");
}

#[test]
fn a_graph_file_that_is_not_there_ends_2_and_says_which() {
    let root = tempdir();
    let out = status(&root, &root.join("nope.toml"), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nope.toml"));
}

#[test]
fn json_is_the_findings_and_the_status_is_the_same() {
    let root = tempdir();
    let graph = world(&root);
    let out = status(&root, &graph, &["--json"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert!(v.as_array().unwrap().iter().all(|f| f["verdict"] == "current"));
    assert!(v.as_array().unwrap().iter().any(|f| f["check"] == "fork-track"));
}

#[test]
fn a_flag_that_belongs_to_another_command_ends_2() {
    let out = Command::new(bin()).args(["init", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn the_manual_and_the_completions_come_from_the_declaration() {
    let man = Command::new(bin()).arg("man").output().unwrap();
    assert!(man.status.success());
    let man = stdout(&man);
    assert!(man.contains("graph") && man.contains("RHIZOID_GRAPH"), "{man}");
    let comp = Command::new(bin()).args(["completions", "bash"]).output().unwrap();
    assert!(comp.status.success() && stdout(&comp).contains("status"));
}
