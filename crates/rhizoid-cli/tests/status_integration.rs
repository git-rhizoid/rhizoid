//! `rhizoid status` over an integration edge: a clone made in a temporary folder, its tags made by the real
//! `rhizoid assemble`, then read against the graph's scheme and topics, its releases and what assemble made.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-status-kernel-test-{}-{}",
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
    let out = Command::new("git").arg("-C").arg(dir).args(args).envs(IDENTITY).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn commit(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", file]);
}

const GRAPH: &str = r#"
[[integration]]
repo = "acme/linux"
topics = ["topic/ITEM", "backport/ITEM/BASE"]
assembled = "int/NAME/LIST/BASE/N"
scheme = { branches = ["main", "topic/ITEM", "backport/ITEM/BASE"], tags = ["int/NAME/LIST/BASE/N"] }
"#;

const LISTS: &str = r#"device = "dev"

[lists.two]
base = "v1.1"

[[lists.two.entries]]
branch = "topic/a"
for = "the a part"
state = "trying"

[[lists.two.entries]]
branch = "backport/b/v1.0"
for = "the b part"
state = "trying"
"#;

struct World {
    root: PathBuf,
    repo: PathBuf,
    graph: PathBuf,
}

/// A kernel clone: releases v1.0 and v1.1, topic/a on v1.1, backport/b/v1.0 on v1.0, both assembled to
/// int/dev/two/v1.1/1 (the default scheme: no int/ branch).
fn world() -> World {
    let root = tempdir();
    let repo = root.join("acme/linux");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, "base.txt", "1.0\n");
    git(&repo, &["tag", "-a", "-m", "v1.0", "v1.0"]);
    commit(&repo, "base.txt", "1.1\n");
    git(&repo, &["tag", "-a", "-m", "v1.1", "v1.1"]);
    git(&repo, &["checkout", "-q", "-b", "topic/a", "v1.1"]);
    commit(&repo, "a.txt", "a\n");
    git(&repo, &["checkout", "-q", "-b", "backport/b/v1.0", "v1.0"]);
    commit(&repo, "b.txt", "b\n");
    git(&repo, &["checkout", "-q", "main"]);
    let graph = root.join("rhizoid.graph.toml");
    std::fs::write(&graph, format!("{GRAPH}releases = \"{}\"\n", repo.display())).unwrap();
    let w = World { root, repo, graph };
    let out = w.assemble();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    w
}

impl World {
    fn assemble(&self) -> Output {
        let lists = self.root.join("topics.toml");
        std::fs::write(&lists, LISTS).unwrap();
        Command::new(bin())
            .args(["assemble", "two", "--lists"])
            .arg(&lists)
            .arg("--repo")
            .arg(&self.repo)
            .env("AGENT_WORKTREES", self.root.join("worktrees"))
            .envs(IDENTITY)
            .output()
            .unwrap()
    }

    fn status(&self) -> (Option<i32>, Vec<serde_json::Value>) {
        let out = Command::new(bin())
            .args(["status", "--json", "--graph"])
            .arg(&self.graph)
            .arg("--root")
            .arg(&self.root)
            .output()
            .unwrap();
        let findings = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stderr)));
        (out.status.code(), findings)
    }

    fn worktree(&self) -> PathBuf {
        self.root.join("worktrees/linux/rhizoid/int-dev-two-v1.1-1")
    }
}

/// The findings of one check, as `verdict: detail`.
fn of(findings: &[serde_json::Value], check: &str) -> Vec<String> {
    findings
        .iter()
        .filter(|f| f["check"] == check)
        .map(|f| format!("{}: {}", f["verdict"].as_str().unwrap(), f["detail"].as_str().unwrap()))
        .collect()
}

#[test]
fn a_clone_as_assemble_left_it_is_current_on_every_check() {
    let w = world();
    let (code, f) = w.status();
    assert_eq!(code, Some(0), "{f:#?}");
    assert_eq!(of(&f, "name-scheme"), ["current: 3 branches and 3 tags, all in the scheme"]);
    assert_eq!(
        of(&f, "topic-base"),
        [
            "current: backport/b/v1.0 is on v1.0, as its name says",
            "current: topic/a is on v1.1, the newest release",
        ]
    );
    assert_eq!(
        of(&f, "recipe"),
        ["current: 2 entries on v1.1; every branch is where the tag took it"]
    );
    let worktree = w.worktree().display().to_string();
    assert_eq!(of(&f, "worktree-tag"), [format!("current: {worktree}: locked, at int/dev/two/v1.1/1 (rhizoid assemble --done int/dev/two/v1.1/1 ends it)")]);
}

#[test]
fn each_sign_of_work_outside_the_flow_is_named() {
    let w = world();
    git(&w.repo, &["branch", "stray", "v1.0"]);
    git(&w.repo, &["tag", "oops", "v1.0"]);
    git(&w.repo, &["checkout", "-q", "-b", "topic/old", "v1.0"]);
    commit(&w.repo, "old.txt", "old\n");
    git(&w.repo, &["branch", "int/dev/two", "main"]);
    git(&w.repo, &["checkout", "-q", "topic/a"]);
    commit(&w.repo, "a.txt", "a, moved on\n");
    git(&w.repo, &["checkout", "-q", "main"]);
    std::fs::write(w.worktree().join("a.txt"), "changed in the worktree\n").unwrap();

    let (code, f) = w.status();
    assert_eq!(code, Some(1), "{f:#?}");
    assert_eq!(
        of(&f, "name-scheme"),
        [
            "moved-by-hand: 2 branches outside the scheme: int/dev/two, stray",
            "moved-by-hand: 1 tags outside the scheme: oops",
        ]
    );
    assert!(
        of(&f, "topic-base")
            .contains(&"behind: topic/old is on v1.0; the newest release is v1.1".to_string()),
        "{f:#?}"
    );
    assert_eq!(of(&f, "recipe"), ["behind: topic/a moved since it was assembled: assemble again"]);
    let worktree = w.worktree().display().to_string();
    assert_eq!(
        of(&f, "worktree-tag"),
        [format!("moved-by-hand: {worktree}: locked, changed beyond int/dev/two/v1.1/1")]
    );
}

#[test]
fn what_the_clone_cannot_say_is_not_known_yet_and_not_drift() {
    let w = world();
    git(&w.repo, &["tag", "-d", "v1.0", "v1.1"]);
    let (code, f) = w.status();
    assert_eq!(code, Some(0), "{f:#?}");
    assert!(of(&f, "topic-base").iter().all(|l| l.starts_with("not-known-yet: ")), "{f:#?}");

    let out = Command::new(bin())
        .args(["status", "--graph"])
        .arg(&w.graph)
        .arg("--root")
        .arg(&w.root)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("3 not known yet (0 drift)"), "{text}");
}

#[test]
fn a_tag_under_int_not_made_by_assemble_is_named() {
    let w = world();
    git(&w.repo, &["tag", "-a", "-m", "made by hand", "int/dev/two/v1.1/2", "main"]);
    let (_, f) = w.status();
    assert_eq!(
        of(&f, "recipe"),
        [
            "current: 2 entries on v1.1; every branch is where the tag took it",
            "moved-by-hand: int/dev/two/v1.1/2 has the shape of an assembled tag but no recipe: made by hand",
        ]
    );
}

#[test]
fn a_worktree_left_at_a_conflict_and_one_locked_for_no_tag_are_named() {
    let w = world();
    let wt = w.worktree();
    git(&w.repo, &["worktree", "unlock", wt.to_str().unwrap()]);
    git(
        &w.repo,
        &[
            "worktree",
            "lock",
            "--reason",
            "rhizoid assemble int/dev/two/v1.1/9: wanted",
            wt.to_str().unwrap(),
        ],
    );
    let left = w.root.join("worktrees/linux/rhizoid/int-dev-two-v1.1-2");
    git(&w.repo, &["worktree", "add", "-q", "--detach", left.to_str().unwrap(), "v1.1"]);
    let (_, f) = w.status();
    assert_eq!(
        of(&f, "worktree-tag"),
        [
            format!("moved-by-hand: {}: locked for int/dev/two/v1.1/9, which was never made", wt.display()),
            format!("behind: {}: unlocked, left at a conflict - resolve and commit there, assemble again, then git worktree remove it", left.display()),
        ]
    );
}

#[test]
fn a_local_topic_behind_origins_is_flagged_and_fetch_keeps_local_int_tags() {
    let w = world();
    let clone = w.root.join("acme/clone");
    let ok = Command::new("git").args(["clone", "-q"]).arg(&w.repo).arg(&clone).status().unwrap();
    assert!(ok.success());
    git(&clone, &["branch", "-q", "topic/a", "origin/topic/a"]);
    git(&clone, &["tag", "int/dev/two/v1.1/5", "v1.1"]);
    git(&clone, &["config", "remote.origin.pruneTags", "true"]);
    git(&w.repo, &["checkout", "-q", "topic/a"]);
    commit(&w.repo, "a.txt", "a, moved on\n");
    git(&w.repo, &["checkout", "-q", "main"]);
    std::fs::write(&w.graph, "[[integration]]\nrepo = \"acme/clone\"\ntopics = [\"topic/ITEM\"]\n")
        .unwrap();

    let out = Command::new(bin())
        .args(["status", "--json", "--fetch", "--graph"])
        .arg(&w.graph)
        .arg("--root")
        .arg(&w.root)
        .output()
        .unwrap();
    let f: Vec<serde_json::Value> =
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {out:?}"));
    assert_eq!(
        of(&f, "branch-origin"),
        ["behind: topic/a is 1 commits behind origin's: assemble would take the stale local one"]
    );
    assert!(
        git(&clone, &["tag", "-l", "int/*"]).contains("int/dev/two/v1.1/5"),
        "a local int tag survives --fetch"
    );
}

#[test]
fn without_a_releases_source_a_topics_base_is_not_known_yet() {
    let w = world();
    std::fs::write(&w.graph, GRAPH).unwrap();
    let (code, f) = w.status();
    assert_eq!(code, Some(0), "{f:#?}");
    assert!(
        of(&f, "topic-base").contains(&"not-known-yet: topic/a: where releases come from is not named (releases = \"owner/repo\" in the edge); the clone's own newest is v1.1".to_string()),
        "{f:#?}"
    );
}

#[test]
fn a_newer_release_the_clone_lacks_makes_a_topic_behind() {
    let w = world();
    let upstream = w.root.join("upstream");
    let ok =
        Command::new("git").args(["clone", "-q"]).arg(&w.repo).arg(&upstream).status().unwrap();
    assert!(ok.success());
    commit(&upstream, "base.txt", "1.2\n");
    git(&upstream, &["tag", "-a", "-m", "v1.2", "v1.2"]);
    std::fs::write(&w.graph, format!("{GRAPH}releases = \"{}\"\n", upstream.display())).unwrap();
    let (code, f) = w.status();
    assert_eq!(code, Some(1), "{f:#?}");
    assert!(
        of(&f, "topic-base").contains(
            &"behind: topic/a is on v1.1; the newest release is v1.2 (not in the clone yet)"
                .to_string()
        ),
        "{f:#?}"
    );
}

#[test]
fn origin_is_asked_even_when_the_clone_tracks_none_of_its_topics() {
    let w = world();
    let clone = w.root.join("acme/narrow");
    let ok = Command::new("git").args(["clone", "-q"]).arg(&w.repo).arg(&clone).status().unwrap();
    assert!(ok.success());
    git(&clone, &["branch", "-q", "topic/a", "origin/topic/a"]);
    git(&clone, &["branch", "-q", "backport/b/v1.0", "origin/backport/b/v1.0"]);
    git(&clone, &["branch", "-q", "topic/mine", "v1.1"]);
    git(&clone, &["config", "remote.origin.fetch", "+refs/heads/main:refs/remotes/origin/main"]);
    for r in ["refs/remotes/origin/topic/a", "refs/remotes/origin/backport/b/v1.0"] {
        git(&clone, &["update-ref", "-d", r]);
    }
    git(&w.repo, &["checkout", "-q", "topic/a"]);
    commit(&w.repo, "a.txt", "a, on origin only\n");
    git(&w.repo, &["checkout", "-q", "main"]);
    std::fs::write(
        &w.graph,
        "[[integration]]\nrepo = \"acme/narrow\"\ntopics = [\"topic/ITEM\", \"backport/ITEM/BASE\"]\n",
    )
    .unwrap();
    let (code, f) = w.status();
    assert_eq!(code, Some(1), "{f:#?}");
    assert_eq!(
        of(&f, "branch-origin"),
        [
            "behind: topic/a is not origin's, whose commit the clone does not have (fetch it): assemble would take the local one",
            "current: 1 topic branches are only in this clone, not on origin (not shared yet): topic/mine",
            "current: 1 topic branches are origin's",
        ]
    );
}

#[test]
fn in_a_shallow_clone_a_release_it_was_never_given_is_not_known_yet() {
    let w = world();
    let upstream = w.root.join("upstream");
    let ok =
        Command::new("git").args(["clone", "-q"]).arg(&w.repo).arg(&upstream).status().unwrap();
    assert!(ok.success());
    commit(&upstream, "base.txt", "1.2\n");
    git(&upstream, &["tag", "-a", "-m", "v1.2", "v1.2"]);
    let shallow = w.root.join("acme/shallow");
    let url = format!("file://{}", w.repo.display());
    let ok = Command::new("git")
        .args(["clone", "-q", "--depth", "1", "--no-single-branch", &url])
        .arg(&shallow)
        .status()
        .unwrap();
    assert!(ok.success());
    git(&shallow, &["branch", "-q", "topic/a", "origin/topic/a"]);
    std::fs::write(
        &w.graph,
        format!("[[integration]]\nrepo = \"acme/shallow\"\ntopics = [\"topic/ITEM\"]\nreleases = \"{}\"\n", upstream.display()),
    )
    .unwrap();
    let (_, f) = w.status();
    assert_eq!(
        of(&f, "topic-base"),
        ["not-known-yet: topic/a: the newest release, v1.2, is not in this shallow clone - whether topic/a is on it is not known yet"]
    );
}
