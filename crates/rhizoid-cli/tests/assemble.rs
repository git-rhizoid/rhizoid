//! `rhizoid assemble` against a real git repository made in a temporary folder: a base tag, topic branches,
//! a lab's list file, and rhizoid's worktree home under its own AGENT_WORKTREES.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-assemble-test-{}-{}",
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

fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).envs(IDENTITY).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git(dir: &Path, args: &[&str]) {
    git_out(dir, args);
}

/// Write the files and commit them on the branch checked out.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str) {
    for (path, text) in files {
        std::fs::write(dir.join(path), text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// A branch from `from` with one commit of these files.
fn branch(dir: &Path, name: &str, from: &str, files: &[(&str, &str)]) {
    git(dir, &["checkout", "-q", "-b", name, from]);
    commit(dir, files, name);
    git(dir, &["checkout", "-q", "main"]);
}

const LISTS: &str = r#"device = "dev"

[lists.two]
base = "v1"

[[lists.two.entries]]
branch = "topic/a"
for = "the a part"
state = "trying"

[[lists.two.entries]]
branch = "topic/b"
for = "the b part"
state = "known-good"
test_run = "run-1"
tested_commit = "TESTED"

[lists.clash]
base = "v1"

[[lists.clash.entries]]
branch = "topic/a"
for = "the a part"
state = "trying"

[[lists.clash.entries]]
branch = "topic/b"
for = "the b part"
state = "trying"

[[lists.clash.entries]]
branch = "topic/clash"
for = "another a"
state = "trying"

[lists.held]
base = "v2"

[[lists.held.entries]]
branch = "topic/a"
for = "the a part"
state = "trying"

[[lists.held.entries]]
branch = "topic/b"
for = "the b part"
state = "trying"
"#;

/// The world: a clone `linux` with tag v1 and three topics, the list file, and the worktree home.
struct World {
    root: PathBuf,
    repo: PathBuf,
    lists: PathBuf,
}

fn world() -> World {
    let root = tempdir();
    let repo = root.join("linux");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, &[("a.txt", "a\n"), ("b.txt", "b\n"), ("c.txt", "c\n")], "base");
    git(&repo, &["tag", "v1"]);
    branch(&repo, "topic/a", "v1", &[("a.txt", "a from topic a\n")]);
    branch(&repo, "topic/b", "v1", &[("b.txt", "b from topic b\n")]);
    branch(&repo, "topic/clash", "v1", &[("a.txt", "a from topic clash\n")]);
    git(&repo, &["tag", "v2", "topic/a"]);
    let lists = root.join("topics.toml");
    let tested = git_out(&repo, &["rev-parse", "topic/b"]);
    std::fs::write(&lists, LISTS.replace("TESTED", &tested.trim()[..12])).unwrap();
    World { root, repo, lists }
}

impl World {
    fn home(&self) -> PathBuf {
        self.root.join("worktrees/linux/rhizoid")
    }

    fn assemble(&self, args: &[&str]) -> Output {
        Command::new(bin())
            .arg("assemble")
            .args(args)
            .arg("--lists")
            .arg(&self.lists)
            .arg("--repo")
            .arg(&self.repo)
            .env("AGENT_WORKTREES", self.root.join("worktrees"))
            .env("HOME", &self.root)
            .envs(IDENTITY)
            .current_dir(&self.root)
            .output()
            .unwrap()
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The value of `key` in key=value output.
fn value(out: &Output, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    stdout(out).lines().find_map(|l| l.strip_prefix(&prefix).map(str::to_string))
}

#[test]
fn two_topics_assemble_to_a_tag_that_holds_both() {
    let w = world();
    let out = w.assemble(&["two"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "tag").as_deref(), Some("int/dev/two/v1/1"));
    assert_eq!(git_out(&w.repo, &["show", "int/dev/two/v1/1:a.txt"]), "a from topic a\n");
    assert_eq!(git_out(&w.repo, &["show", "int/dev/two/v1/1:b.txt"]), "b from topic b\n");

    let base_sha = git_out(&w.repo, &["rev-parse", "v1^{commit}"]);
    let a_sha = git_out(&w.repo, &["rev-parse", "topic/a"]);
    assert_eq!(value(&out, "base_sha").as_deref(), Some(base_sha.trim()));
    assert_eq!(value(&out, "entry.1.branch").as_deref(), Some("topic/a"));
    assert_eq!(value(&out, "entry.1.sha").as_deref(), Some(a_sha.trim()));
    assert_eq!(value(&out, "entry.2.state").as_deref(), Some("known-good"));
    let b_sha = git_out(&w.repo, &["rev-parse", "topic/b"]);
    assert_eq!(value(&out, "entry.2.tested_commit").as_deref(), Some(b_sha.trim()), "the full sha");
    assert_eq!(value(&out, "tested").as_deref(), Some(b_sha.trim()));
    assert_eq!(value(&out, "topic.a").as_deref(), Some(a_sha.trim()));
    assert_eq!(value(&out, "topic.b").as_deref(), Some(b_sha.trim()));
    assert_eq!(value(&out, "entry.1.source").as_deref(), Some("local"));
    assert_eq!(value(&out, "unchanged"), None);

    let message = git_out(&w.repo, &["tag", "-l", "--format=%(contents)", "int/dev/two/v1/1"]);
    assert!(message.contains(&format!("base_sha={}", base_sha.trim())), "{message}");
    assert!(message.contains(&format!("entry.1.sha={}", a_sha.trim())), "{message}");
    let tag_sha = git_out(&w.repo, &["rev-parse", "refs/tags/int/dev/two/v1/1"]);
    assert_eq!(value(&out, "tag_sha").as_deref(), Some(tag_sha.trim()));

    let worktree = value(&out, "worktree").unwrap();
    assert_eq!(PathBuf::from(&worktree), w.home().join("int-dev-two-v1-1"));
    let list = git_out(&w.repo, &["worktree", "list", "--porcelain"]);
    let entry = list.split("\n\n").find(|e| e.contains(&worktree)).expect("the worktree is git's");
    assert!(entry.contains("locked"), "{entry}");
}

#[test]
fn a_conflict_stops_and_names_both_entries() {
    let w = world();
    let out = w.assemble(&["clash"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "conflict.entry").as_deref(), Some("topic/clash"));
    assert_eq!(
        value(&out, "conflict.with").as_deref(),
        Some("topic/a"),
        "topic/b touched other paths"
    );
    assert_eq!(value(&out, "conflict.path.1").as_deref(), Some("a.txt"));
    assert_eq!(value(&out, "tag"), None);
    assert_eq!(git_out(&w.repo, &["tag", "-l", "int/*"]), "");
    let worktree = value(&out, "worktree").unwrap();
    let list = git_out(&w.repo, &["worktree", "list", "--porcelain"]);
    let entry = list.split("\n\n").find(|e| e.contains(&worktree)).expect("the worktree is git's");
    assert!(!entry.contains("locked"), "left unlocked for a hand to resolve: {entry}");
}

#[test]
fn a_resolution_made_by_hand_is_replayed_by_rerere_on_the_next_run() {
    let w = world();
    let out = w.assemble(&["clash"]);
    let worktree = PathBuf::from(value(&out, "worktree").unwrap());
    std::fs::write(worktree.join("a.txt"), "a, both\n").unwrap();
    git(&worktree, &["add", "a.txt"]);
    git(&worktree, &["commit", "-q", "--no-edit"]);

    let again = w.assemble(&["clash"]);
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert_eq!(value(&again, "entry.3.merged").as_deref(), Some("rerere-resolved"));
    let message = git_out(&w.repo, &["log", "-1", "--format=%B", "int/dev/clash/v1/1"]);
    assert!(!message.contains('#'), "no comment lines in the merge: {message}");
    assert!(
        message.starts_with("rhizoid assemble int/dev/clash/v1/1: merge topic/clash"),
        "{message}"
    );
    assert_eq!(value(&again, "tag").as_deref(), Some("int/dev/clash/v1/1"));
    assert_eq!(git_out(&w.repo, &["show", "int/dev/clash/v1/1:a.txt"]), "a, both\n");
    assert_ne!(PathBuf::from(value(&again, "worktree").unwrap()), worktree, "a fresh worktree");
}

#[test]
fn reproduces_says_equal_or_lists_the_paths_that_differ() {
    let w = world();
    git(&w.repo, &["checkout", "-q", "-b", "by-hand", "v1"]);
    git(&w.repo, &["merge", "-q", "--no-edit", "topic/a", "topic/b"]);
    let by_hand = git_out(&w.repo, &["rev-parse", "HEAD"]).trim().to_string();
    commit(&w.repo, &[("c.txt", "c changed by hand\n")], "by hand");
    let changed = git_out(&w.repo, &["rev-parse", "HEAD"]).trim().to_string();
    git(&w.repo, &["checkout", "-q", "main"]);

    let out = w.assemble(&["two", "--reproduces", &by_hand]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "reproduces").as_deref(), Some("equal"));

    let out = w.assemble(&["two", "--reproduces", &changed]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "reproduces").as_deref(), Some("differs"));
    assert_eq!(value(&out, "reproduces.path.1").as_deref(), Some("c.txt"));
    assert_eq!(value(&out, "reproduces.path.2"), None);
}

#[test]
fn a_worktree_outside_the_home_is_refused() {
    let w = world();
    let outside = w.root.join("elsewhere");
    let dotted = w.home().join("../../../elsewhere");
    for wanted in [&outside, &dotted] {
        let out = w.assemble(&["two", "--worktree", wanted.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(2), "{out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("outside rhizoid's home"), "{out:?}");
    }
    assert!(!outside.exists());
    assert_eq!(git_out(&w.repo, &["tag", "-l", "int/*"]), "");

    let inside = w.home().join("mine");
    let out = w.assemble(&["two", "--worktree", inside.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "worktree").map(PathBuf::from), Some(inside));
}

#[test]
fn an_unchanged_list_is_its_tag_and_a_changed_one_the_next_number() {
    let w = world();
    let first = w.assemble(&["two"]);
    let second = w.assemble(&["two"]);
    assert_eq!(second.status.code(), Some(0), "{second:?}");
    assert_eq!(value(&second, "tag"), value(&first, "tag"));
    assert_eq!(value(&second, "tag_sha"), value(&first, "tag_sha"));
    assert_eq!(value(&second, "unchanged").as_deref(), Some("yes"));
    assert_eq!(value(&second, "worktree"), value(&first, "worktree"), "its worktree, reused");
    assert_eq!(git_out(&w.repo, &["tag", "-l", "int/*"]).lines().count(), 1);

    git(&w.repo, &["checkout", "-q", "topic/a"]);
    commit(&w.repo, &[("a.txt", "a, again\n")], "again");
    git(&w.repo, &["checkout", "-q", "main"]);
    let third = w.assemble(&["two"]);
    assert_eq!(third.status.code(), Some(0), "{third:?}");
    assert_eq!(value(&third, "tag").as_deref(), Some("int/dev/two/v1/2"));
    assert_eq!(
        value(&third, "worktree").map(PathBuf::from),
        Some(w.home().join("int-dev-two-v1-2"))
    );
    assert!(w.home().join("int-dev-two-v1-1").is_dir(), "several assembled worktrees stay alive");
}

#[test]
fn json_carries_the_same_report() {
    let w = world();
    let out = w.assemble(&["two", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["tag"], "int/dev/two/v1/1");
    assert_eq!(v["base"], "v1");
    assert_eq!(v["entries"][0]["branch"], "topic/a");
    assert_eq!(v["entries"][1]["state"], "known-good");
    assert_eq!(v["entries"][1]["test_run"], "run-1");
}

#[test]
fn a_missing_branch_or_list_stops_before_anything_is_made() {
    let w = world();
    git(&w.repo, &["branch", "-q", "-D", "topic/b"]);
    let out = w.assemble(&["two"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no branch topic/b"), "{out:?}");
    let out = w.assemble(&["nope"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("the file has clash, held, two"),
        "{out:?}"
    );
    assert!(!w.home().join("int-dev-two-v1-1").exists());
}

#[test]
fn fetch_prunes_a_branch_the_remote_no_longer_has() {
    let w = world();
    let clone = w.root.join("clone/linux");
    let status =
        Command::new("git").args(["clone", "-q"]).arg(&w.repo).arg(&clone).status().unwrap();
    assert!(status.success());
    git(&clone, &["fetch", "-q", "--tags"]);
    git(&w.repo, &["branch", "-q", "-D", "topic/clash"]);
    assert!(git_out(&clone, &["branch", "-r"]).contains("origin/topic/clash"));
    let out = Command::new(bin())
        .args(["assemble", "two", "--fetch", "--lists"])
        .arg(&w.lists)
        .arg("--repo")
        .arg(&clone)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(!git_out(&clone, &["branch", "-r"]).contains("origin/topic/clash"), "pruned");
    assert_eq!(
        value(&out, "entry.1.source").as_deref(),
        Some("origin"),
        "no local topic/a in the clone"
    );

    git(&clone, &["config", "remote.origin.pruneTags", "true"]);
    let again = Command::new(bin())
        .args(["assemble", "two", "--fetch", "--lists"])
        .arg(&w.lists)
        .arg("--repo")
        .arg(&clone)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert!(
        git_out(&clone, &["tag", "-l", "int/*"]).contains("int/dev/two/v1/1"),
        "a local tag survives pruneTags"
    );
}

#[test]
fn an_entry_the_base_already_holds_is_said_so_not_merged() {
    let w = world();
    let out = w.assemble(&["held"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "entry.1.merged").as_deref(), Some("already-in"));
    assert_eq!(value(&out, "entry.2.merged").as_deref(), Some("merged"));
}

#[test]
fn a_merge_git_refuses_says_gits_reason() {
    let w = world();
    git(&w.repo, &["config", "user.useConfigOnly", "true"]);
    let out = Command::new(bin())
        .args(["assemble", "two", "--lists"])
        .arg(&w.lists)
        .arg("--repo")
        .arg(&w.repo)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .env("HOME", &w.root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("EMAIL")
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("failed before merging") && err.contains("identity"), "{err}");
}

#[test]
fn the_home_is_named_after_the_main_clone_from_a_linked_worktree() {
    let w = world();
    let linked = w.root.join("linked");
    git(&w.repo, &["worktree", "add", "-q", "--detach", linked.to_str().unwrap(), "v1"]);
    let out = Command::new(bin())
        .args(["assemble", "two", "--lists"])
        .arg(&w.lists)
        .arg("--repo")
        .arg(&linked)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "worktree").map(PathBuf::from), Some(w.home().join("int-dev-two-v1-1")));
}

impl World {
    fn done(&self, args: &[&str]) -> Output {
        Command::new(bin())
            .args(["assemble", "--done"])
            .args(args)
            .arg("--repo")
            .arg(&self.repo)
            .output()
            .unwrap()
    }
}

#[test]
fn done_ends_a_tags_worktree_and_refuses_one_with_changes_unless_forced() {
    let w = world();
    let out = w.assemble(&["two"]);
    let worktree = PathBuf::from(value(&out, "worktree").unwrap());
    std::fs::write(worktree.join("a.txt"), "changed\n").unwrap();
    let refused = w.done(&["int/dev/two/v1/1"]);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--force"), "{refused:?}");
    assert!(worktree.is_dir());

    let forced = w.done(&["int/dev/two/v1/1", "--force"]);
    assert_eq!(forced.status.code(), Some(0), "{forced:?}");
    assert_eq!(value(&forced, "removed").map(PathBuf::from), Some(worktree.clone()));
    assert!(!worktree.exists());
    assert!(!git_out(&w.repo, &["worktree", "list"]).contains(worktree.to_str().unwrap()));
    assert!(git_out(&w.repo, &["tag", "-l"]).contains("int/dev/two/v1/1"), "the tag stays");

    let again = w.assemble(&["two"]);
    assert_eq!(value(&again, "unchanged").as_deref(), Some("yes"));
    assert_eq!(value(&again, "worktree"), None, "its worktree is gone; the tag is the result");
    let clean = w.done(&["int/dev/two/v1/9"]);
    assert_eq!(clean.status.code(), Some(2), "{clean:?}");
}

#[test]
fn a_tested_commit_must_be_a_commits_sha() {
    let w = world();
    for name in ["HEAD", "topic/b", "abcdef0123"] {
        let text = std::fs::read_to_string(&w.lists).unwrap();
        let tested = text.lines().find(|l| l.starts_with("tested_commit")).unwrap().to_string();
        std::fs::write(&w.lists, text.replace(&tested, &format!("tested_commit = \"{name}\"")))
            .unwrap();
        let out = w.assemble(&["two"]);
        assert_eq!(out.status.code(), Some(2), "{name}: {out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("is not a commit's sha"), "{out:?}");
    }
}

#[test]
fn any_repository_names_its_own_tags_and_branches() {
    let w = world();
    std::fs::write(
        &w.lists,
        r#"tag = "candidate/{list}-{n}"

[lists.release]
base = "v1"

[[lists.release.entries]]
branch = "topic/a"
for = "the a part"
state = "trying"
"#,
    )
    .unwrap();
    let out = w.assemble(&["release"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "tag").as_deref(), Some("candidate/release-1"));
    assert_eq!(value(&out, "name"), None, "no name: none printed");
    assert_eq!(
        value(&out, "worktree").map(PathBuf::from),
        Some(w.home().join("candidate-release-1"))
    );
    git(&w.repo, &["checkout", "-q", "topic/a"]);
    commit(&w.repo, &[("a.txt", "a, again\n")], "again");
    git(&w.repo, &["checkout", "-q", "main"]);
    assert_eq!(value(&w.assemble(&["release"]), "tag").as_deref(), Some("candidate/release-2"));
}

#[test]
fn the_list_file_has_no_default() {
    let w = world();
    let out = Command::new(bin())
        .args(["assemble", "two", "--repo"])
        .arg(&w.repo)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .env_remove("RHIZOID_LISTS")
        .current_dir(&w.root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("--lists FILE"), "{out:?}");
}

#[test]
fn done_refuses_a_worktree_with_commits_beyond_its_tag() {
    let w = world();
    let out = w.assemble(&["two"]);
    let worktree = PathBuf::from(value(&out, "worktree").unwrap());
    commit(&worktree, &[("c.txt", "committed in the worktree\n")], "beyond");
    let refused = w.done(&["int/dev/two/v1/1"]);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(String::from_utf8_lossy(&refused.stderr).contains("commits beyond"), "{refused:?}");
    assert!(worktree.is_dir());
}

#[test]
fn a_list_whose_base_or_entry_is_a_reference_tag_of_the_graph_is_refused() {
    let w = world();
    let sha = git_out(&w.repo, &["rev-parse", "v1^{commit}"]).trim().to_string();
    let graph = w.root.join("graph.toml");
    let edge = |tag: &str| {
        format!(
            "[[reference]]\nrepo = \"acme/linux\"\nname = \"vendor\"\nurl = \"https://example.org/vendor\"\nref = \"build-1\"\ncommit = \"{sha}\"\ntag = \"{tag}\"\nwhy = \"the vendor's kernel at the build\"\n"
        )
    };
    std::fs::write(&graph, edge("v1")).unwrap();
    let out = w.assemble(&["two", "--graph", graph.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("the base v1 is a reference tag"),
        "{out:?}"
    );

    std::fs::write(&graph, edge("topic/b")).unwrap();
    let out = w.assemble(&["two", "--graph", graph.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("the entry topic/b is a reference tag"),
        "{out:?}"
    );

    std::fs::write(&graph, edge("reference/vendor/build-1")).unwrap();
    let out = w.assemble(&["two", "--graph", graph.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "no reference in the list: {out:?}");
}
