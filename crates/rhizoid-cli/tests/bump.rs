//! `rhizoid bump` against real repositories in a temporary folder: a tools source with releases, a slice source,
//! and a follower cloned from a bare origin that pins both tools (one through a Cargo git tag, gated) and the slice.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-bump-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

const IDENTITY: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@example.com"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@example.com"),
];

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgSign=false"])
        .args(args)
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn repo(dir: &Path, files: &[(&str, &str)], tags: &[&str]) {
    std::fs::create_dir_all(dir).unwrap();
    if !dir.join(".git").exists() {
        git(dir, &["init", "-q", "-b", "main"]);
    }
    for (path, text) in files {
        std::fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
        std::fs::write(dir.join(path), text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "change"]);
    for t in tags {
        git(dir, &["tag", t]);
    }
}

const CARGO: &str = "[dependencies]\nlpoco-attended = { git = \"https://example.org/lab-tools\", tag = \"lpoco-attended-v0.1.0\" }\n";

struct World {
    root: PathBuf,
    lab: PathBuf,
    bare: PathBuf,
    graph: PathBuf,
}

fn world(gate: &str) -> World {
    let root = tempdir();
    let tools = root.join("acme/tools");
    repo(&tools, &[("a", "1")], &["lpoco-v1.0.0", "lpoco-attended-v0.1.0"]);
    repo(&tools, &[("a", "2")], &["lpoco-v1.1.0", "lpoco-attended-v0.2.0"]);
    repo(
        &root.join("acme/kb"),
        &[("generic", "queries/k.sql\n"), ("queries/k.sql", "-- k v1\n")],
        &["v0.1.0"],
    );
    repo(&root.join("acme/kb"), &[("queries/k.sql", "-- k v2\n")], &["v0.2.0"]);
    let seed = root.join("seed");
    repo(
        &seed,
        &[
            ("lab.toml", "# the lab\n[tools]\nlpoco = \"1.0.0\"\nother = \"1.0.0-x\"\n"),
            ("steps/Cargo.toml", CARGO),
            ("idealkb.toml", "source = \"acme/kb\"\nversion = \"0.1.0\"\nown = []\n"),
            ("queries/k.sql", "-- k v1\n"),
        ],
        &[],
    );
    let bare = root.join("bare/lab.git");
    let ok =
        Command::new("git").args(["clone", "-q", "--bare"]).arg(&seed).arg(&bare).status().unwrap();
    assert!(ok.success());
    let lab = root.join("acme/lab");
    let ok = Command::new("git").args(["clone", "-q"]).arg(&bare).arg(&lab).status().unwrap();
    assert!(ok.success());
    let graph = root.join("graph.toml");
    std::fs::write(
        &graph,
        format!(
            r#"[[slice]]
source = "acme/kb"
pin = "idealkb.toml"
marker = "kb"
followers = ["acme/lab"]

[[tool]]
name = "lpoco"
source = "acme/tools"
tag_prefix = "lpoco-v"
pin = "lab.toml"
key = "tools.lpoco"
followers = ["acme/lab"]

[[tool]]
name = "lpoco-attended"
source = "acme/tools"
tag_prefix = "lpoco-attended-v"
pin = "steps/Cargo.toml"
key = "dependencies.lpoco-attended.tag"
value = "lpoco-attended-v{{version}}"
gate = {gate}
followers = ["acme/lab"]
"#
        ),
    )
    .unwrap();
    World { root, lab, bare, graph }
}

impl World {
    fn bump(&self, args: &[&str]) -> Output {
        Command::new(bin())
            .arg("bump")
            .args(args)
            .arg("--graph")
            .arg(&self.graph)
            .arg("--root")
            .arg(&self.root)
            .env("AGENT_WORKTREES", self.root.join("worktrees"))
            .envs(IDENTITY)
            .output()
            .unwrap()
    }
}

fn value(out: &Output, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix(&prefix).map(str::to_string))
}

const PASSING_GATE: &str = r#"["sh", "-c", "grep -q 'lpoco-attended-v0.2.0' steps/Cargo.toml"]"#;

#[test]
fn every_pin_is_listed_in_one_place_and_nothing_changes() {
    let w = world(PASSING_GATE);
    let out = w.bump(&[]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "pin.1.name").as_deref(), Some("lpoco"));
    assert_eq!(value(&out, "pin.1.pinned").as_deref(), Some("1.0.0"));
    assert_eq!(value(&out, "pin.1.latest").as_deref(), Some("1.1.0"));
    assert_eq!(value(&out, "pin.1.state").as_deref(), Some("behind"));
    assert_eq!(value(&out, "pin.2.pinned").as_deref(), Some("0.1.0"), "the Cargo tag's version");
    assert_eq!(value(&out, "pin.3.kind").as_deref(), Some("slice"));
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/*"]), "");
}

#[test]
fn now_bumps_each_tool_pin_on_a_branch_with_only_its_value_changed() {
    let w = world(PASSING_GATE);
    let out = w.bump(&["now"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "pin.1.state").as_deref(), Some("bumped"));
    assert_eq!(value(&out, "pin.1.branch").as_deref(), Some("rhizoid/bump/lpoco-1.1.0"));
    assert_eq!(value(&out, "pin.1.pushed").as_deref(), Some("no"));
    assert_eq!(value(&out, "pin.2.state").as_deref(), Some("bumped"), "the gate passed");
    assert_eq!(value(&out, "pin.3.state").as_deref(), Some("bumped"), "a slice, through follow");
    assert_eq!(value(&out, "pin.3.branch").as_deref(), Some("rhizoid/bump/acme-kb-0.2.0"));
    assert_eq!(git(&w.lab, &["show", "rhizoid/bump/acme-kb-0.2.0:queries/k.sql"]), "-- k v2");
    assert!(git(&w.lab, &["show", "rhizoid/bump/acme-kb-0.2.0:idealkb.toml"])
        .contains("version = \"0.2.0\""));
    assert_eq!(
        git(&w.lab, &["log", "-1", "--format=%s", "rhizoid/bump/acme-kb-0.2.0"]),
        "chore(deps): follow acme/kb 0.1.0 -> 0.2.0"
    );
    assert_eq!(
        git(&w.lab, &["show", "rhizoid/bump/lpoco-1.1.0:lab.toml"]),
        "# the lab\n[tools]\nlpoco = \"1.1.0\"\nother = \"1.0.0-x\""
    );
    let cargo = git(&w.lab, &["show", "rhizoid/bump/lpoco-attended-0.2.0:steps/Cargo.toml"]);
    assert!(
        cargo.contains("tag = \"lpoco-attended-v0.2.0\"")
            && cargo.contains("example.org/lab-tools"),
        "{cargo}"
    );
    assert_eq!(
        git(&w.lab, &["log", "-1", "--format=%s", "rhizoid/bump/lpoco-1.1.0"]),
        "chore(deps): lpoco 1.0.0 -> 1.1.0"
    );
    assert_eq!(
        git(&w.lab, &["diff", "--name-only", "origin/main", "rhizoid/bump/lpoco-1.1.0"]),
        "lab.toml"
    );
    assert_eq!(
        git(&w.lab, &["status", "--porcelain"]),
        "",
        "the follower's own checkout untouched"
    );
    assert!(
        !w.root.join("worktrees/lab/rhizoid/rhizoid-bump-lpoco-1.1.0").exists(),
        "the worktree goes"
    );
    assert_eq!(git(&w.bare, &["branch", "--list", "rhizoid/*"]), "", "nothing pushed unasked");

    let again = w.bump(&["now"]);
    assert_eq!(value(&again, "pin.1.state").as_deref(), Some("branch-exists"));
}

#[test]
fn a_failing_gate_commits_nothing() {
    let w = world(r#"["sh", "-c", "echo 'the attended check says no'; exit 4"]"#);
    let out = w.bump(&["now"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "pin.2.state").as_deref(), Some("gated-out"));
    assert_eq!(value(&out, "pin.2.gate.exit").as_deref(), Some("4"));
    assert_eq!(value(&out, "pin.2.gate.output").as_deref(), Some("the attended check says no"));
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/bump/lpoco-attended*"]), "");
    assert_eq!(value(&out, "pin.1.state").as_deref(), Some("bumped"), "the other pin is its own");
}

#[test]
fn push_sends_the_branch_to_origin_and_only_there() {
    let w = world(PASSING_GATE);
    let out = w.bump(&["now", "--push", "--only", "acme/lab"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "pin.1.pushed").as_deref(), Some("yes"));
    assert!(git(&w.bare, &["branch", "--list", "rhizoid/*"]).contains("rhizoid/bump/lpoco-1.1.0"));
    assert_eq!(
        git(&w.bare, &["rev-parse", "main"]),
        git(&w.lab, &["rev-parse", "origin/main"]),
        "main untouched"
    );
}

#[test]
fn a_value_that_is_not_certain_is_left_to_a_hand() {
    let w = world(PASSING_GATE);
    std::fs::write(
        w.lab.join("lab.toml"),
        "[tools]\nlpoco = \"1.0.0\"\n[also]\nlpoco = \"1.0.0\"\n",
    )
    .unwrap();
    git(&w.lab, &["commit", "-q", "-am", "twice"]);
    git(&w.lab, &["push", "-q", "origin", "main"]);
    git(&w.lab, &["fetch", "-q"]);
    let out = w.bump(&["now", "--only", "acme/lab"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "pin.1.state").as_deref(), Some("refused"));
    assert!(value(&out, "pin.1.why").unwrap().contains("2 times"), "{out:?}");
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/bump/lpoco-1*"]), "");
}

#[test]
fn a_gate_that_changes_more_than_the_pin_is_refused() {
    let w =
        world(r#"["sh", "-c", "echo built >> lab.toml.lock && echo more >> steps/Cargo.toml"]"#);
    let out = w.bump(&["now"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "pin.2.state").as_deref(), Some("refused"));
    assert!(value(&out, "pin.2.why").unwrap().contains("lab.toml.lock"), "{out:?}");
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/bump/lpoco-attended*"]), "");
}

#[test]
fn pushing_needs_now_and_only_must_name_a_follower() {
    let w = world(PASSING_GATE);
    let out = w.bump(&["--push"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("go with `bump now`"), "{out:?}");
    let out = w.bump(&["now", "--only", "acme/lsb"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no pin of the graph"), "{out:?}");
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/*"]), "");
}

#[test]
fn a_failed_pull_request_keeps_the_pushed_branch_and_says_so() {
    let w = world(PASSING_GATE);
    let fake = w.root.join("fakebin");
    std::fs::create_dir_all(&fake).unwrap();
    std::fs::write(fake.join("gh"), "#!/bin/sh\necho 'gh: not logged in' >&2\nexit 4\n").unwrap();
    let ok = Command::new("chmod").arg("+x").arg(fake.join("gh")).status().unwrap();
    assert!(ok.success());
    let path = format!("{}:{}", fake.display(), std::env::var("PATH").unwrap_or_default());
    let out = Command::new(bin())
        .args(["bump", "now", "--pr", "--only", "acme/lab", "--graph"])
        .arg(&w.graph)
        .arg("--root")
        .arg(&w.root)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .env("PATH", path)
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "pin.1.pushed").as_deref(), Some("yes"));
    assert_eq!(value(&out, "pin.1.pr_error").as_deref(), Some("gh: not logged in"));
    assert!(
        git(&w.lab, &["branch", "--list", "rhizoid/*"]).contains("rhizoid/bump/lpoco-1.1.0"),
        "kept"
    );
    assert!(git(&w.bare, &["branch", "--list", "rhizoid/*"]).contains("rhizoid/bump/lpoco-1.1.0"));
    let again = w.bump(&["now", "--only", "acme/lab"]);
    assert_eq!(value(&again, "pin.1.state").as_deref(), Some("branch-exists"));
}

#[test]
fn what_the_gate_may_change_is_committed_with_the_pin() {
    let w = world(r#"["sh", "-c", "echo 'tag = 0.2.0' > steps/Cargo.lock"]"#);
    let graph = std::fs::read_to_string(&w.graph).unwrap();
    std::fs::write(
        &w.graph,
        graph.replace("gate = [\"sh\"", "also = [\"steps/Cargo.lock\"]\ngate = [\"sh\""),
    )
    .unwrap();
    let out = w.bump(&["now", "--only", "acme/lab"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "pin.2.state").as_deref(), Some("bumped"));
    let files =
        git(&w.lab, &["diff", "--name-only", "origin/main", "rhizoid/bump/lpoco-attended-0.2.0"]);
    assert_eq!(files, "steps/Cargo.lock\nsteps/Cargo.toml");
}

#[test]
fn a_follower_that_changed_a_generic_file_is_left_for_a_hand() {
    let w = world(PASSING_GATE);
    std::fs::write(w.lab.join("queries/k.sql"), "-- k, changed by the lab\n").unwrap();
    git(&w.lab, &["commit", "-q", "-am", "the lab's own k"]);
    git(&w.lab, &["push", "-q", "origin", "main"]);
    git(&w.lab, &["fetch", "-q"]);
    let out = w.bump(&["now", "--only", "acme/lab"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "pin.3.state").as_deref(), Some("refused"));
    assert!(value(&out, "pin.3.why").unwrap().contains("changed a generic file"), "{out:?}");
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/bump/kb*"]), "");
}

#[test]
fn a_slice_gate_that_fails_commits_nothing() {
    let w = world(PASSING_GATE);
    let graph = std::fs::read_to_string(&w.graph).unwrap();
    std::fs::write(
        &w.graph,
        graph
            .replace("marker = \"kb\"\n", "marker = \"kb\"\ngate = [\"sh\", \"-c\", \"exit 6\"]\n"),
    )
    .unwrap();
    let out = w.bump(&["now", "--only", "acme/lab"]);
    assert_eq!(value(&out, "pin.3.state").as_deref(), Some("gated-out"), "{out:?}");
    assert_eq!(value(&out, "pin.3.gate.exit").as_deref(), Some("6"));
    assert_eq!(git(&w.lab, &["branch", "--list", "rhizoid/bump/kb*"]), "");
}

#[test]
fn a_slice_release_naming_a_path_outside_the_follower_is_not_laid() {
    let w = world(PASSING_GATE);
    repo(
        &w.root.join("acme/kb"),
        &[("generic", "queries/k.sql\n../escape.txt\n"), ("queries/k.sql", "-- k v3\n")],
        &["v0.3.0"],
    );
    let out = w.bump(&["now", "--only", "acme/lab"]);
    assert_eq!(value(&out, "pin.3.state").as_deref(), Some("refused"), "{out:?}");
    assert!(value(&out, "pin.3.why").unwrap().contains("outside the follower"), "{out:?}");
    assert!(!w.root.join("acme/escape.txt").exists());
}

#[test]
fn a_cuttings_pin_moves_to_its_newest_cut_never_to_a_plain_upstream_tag() {
    let w = world(PASSING_GATE);
    git(&w.root.join("acme/tools"), &["tag", "lpoco-v1.0.0-cut.3"]);
    git(&w.root.join("acme/tools"), &["tag", "lpoco-v1.0.0-cut.10"]);
    let graph = std::fs::read_to_string(&w.graph).unwrap();
    std::fs::write(
        &w.graph,
        graph.replace(
            "tag_prefix = \"lpoco-v\"\n",
            "tag_prefix = \"lpoco-v\"\ntag_suffix = \"-cut.N\"\n",
        ),
    )
    .unwrap();
    let out = w.bump(&["--only", "acme/lab"]);
    assert_eq!(value(&out, "pin.1.latest").as_deref(), Some("1.0.0-cut.10"), "{out:?}");
}
