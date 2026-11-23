//! `rhizoid send-back` against a real repository made in a temporary folder: a release tag, a target tree that
//! moved on (`for-next`), a topic branch, and checks and recipients configured as plain commands.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-send-back-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

const IDENTITY: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Author One"),
    ("GIT_AUTHOR_EMAIL", "one@example.com"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@example.com"),
];

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgSign=false", "-c", "tag.gpgSign=false"])
        .args(args)
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, file: &str, text: &str, subject: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", subject]);
}

const CONFIG: &str = r#"
[[send.check]]
name = "builds"
run = ["sh", "-c", "exit 0"]

[[send.check]]
name = "patches-exist"
run = ["sh", "-c", "for f; do test -f \"$f\" || exit 9; done; test $# -eq 2", "sh"]
patches = true

[send.recipients]
name = "maintainers"
run = ["printf", "Maint One <m1@example.org>\nlist@example.org\n"]
patches = true
"#;

struct World {
    root: PathBuf,
    repo: PathBuf,
    config: PathBuf,
}

/// v1; for-next one commit past it; topic/x two commits on v1; signing configured to fail if anything signs.
fn world() -> World {
    let root = tempdir();
    let repo = root.join("app");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, "base.txt", "base\n", "base");
    git(&repo, &["tag", "v1"]);
    git(&repo, &["checkout", "-q", "-b", "for-next", "v1"]);
    commit(&repo, "next.txt", "next\n", "next: a change in the target tree");
    git(&repo, &["checkout", "-q", "-b", "topic/x", "v1"]);
    commit(&repo, "x.txt", "x\n", "x: add x");
    commit(&repo, "x.txt", "x, better\n", "x: make x better");
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["config", "commit.gpgSign", "true"]);
    git(&repo, &["config", "gpg.program", "false"]);
    git(&repo, &["config", "tag.gpgSign", "true"]);
    let config = root.join("rhizoid.toml");
    std::fs::write(&config, CONFIG).unwrap();
    World { root, repo, config }
}

impl World {
    fn send(&self, out: &str, extra: &[&str]) -> Output {
        Command::new(bin())
            .args(["send-back", "topic/x", "--onto", "for-next", "--out"])
            .arg(self.root.join(out))
            .arg("--repo")
            .arg(&self.repo)
            .arg("--config")
            .arg(&self.config)
            .args(extra)
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

#[test]
fn a_branch_becomes_a_tagged_version_cut_onto_the_target_and_checked() {
    let w = world();
    let tested = git(&w.repo, &["rev-parse", "topic/x"]);
    let out = w.send("series/v1", &["--tested", &tested[..10], "--fork", "acme-app"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "version").as_deref(), Some("v1"));
    assert_eq!(value(&out, "carried-in").as_deref(), Some("acme-app/topic/x"));
    assert_eq!(value(&out, "local-ref").as_deref(), Some("topic/x/v1"));
    assert_eq!(value(&out, "file").map(PathBuf::from), Some(w.root.join("series/v1")));
    assert_eq!(value(&out, "tested").as_deref(), Some(tested.as_str()), "the full sha");
    assert_eq!(value(&out, "range_diff").as_deref(), Some("none"));
    assert_eq!(value(&out, "check.2.name").as_deref(), Some("patches-exist"));
    assert_eq!(value(&out, "check.2.exit").as_deref(), Some("0"));
    assert_eq!(value(&out, "recipient.1").as_deref(), Some("Maint One <m1@example.org>"));
    assert_eq!(value(&out, "recipient.2").as_deref(), Some("list@example.org"));

    let next = git(&w.repo, &["rev-parse", "for-next"]);
    assert_eq!(git(&w.repo, &["rev-parse", "send/x/v1~2"]), next, "rebased onto the target");
    assert_eq!(
        git(&w.repo, &["rev-parse", "topic/x/v1^{commit}"]),
        git(&w.repo, &["rev-parse", "send/x/v1"])
    );
    assert_eq!(
        git(&w.repo, &["log", "-1", "--format=%an %ae", "send/x/v1"]),
        "Author One one@example.com"
    );
    assert_eq!(git(&w.repo, &["log", "-1", "--format=%G?", "send/x/v1"]), "N", "nothing signed");

    let files: Vec<String> = (1..=3).filter_map(|i| value(&out, &format!("patch.{i}"))).collect();
    assert_eq!(files.len(), 3, "{out:?}");
    let cover = std::fs::read_to_string(&files[0]).unwrap();
    assert!(files[0].ends_with("v1-0000-cover-letter.patch"), "{files:?}");
    assert!(cover.contains("Changes in v1:"), "{cover}");
    assert!(cover.contains(&format!("base-commit: {next}")), "{cover}");
    let first = std::fs::read_to_string(&files[1]).unwrap();
    assert!(!first.contains("Signed-off-by"), "nothing signed off");
    let notes = std::fs::read_to_string(w.root.join("series/v1/SEND-NOTES")).unwrap();
    assert!(
        notes.contains(&format!("tested={tested}")) && notes.contains(&format!("base_sha={next}")),
        "{notes}"
    );
    assert!(!w.root.join("worktrees/app/rhizoid/send-x-v1").exists(), "the worktree goes");
}

#[test]
fn the_next_version_carries_its_range_diff_and_a_changelog_to_fill() {
    let w = world();
    assert_eq!(w.send("series/v1", &[]).status.code(), Some(0));
    git(&w.repo, &["checkout", "-q", "topic/x"]);
    commit(&w.repo, "x.txt", "x, best\n", "x: make x best");
    git(&w.repo, &["checkout", "-q", "main"]);
    std::fs::write(&w.config, CONFIG.replace("test $# -eq 2", "test $# -eq 3")).unwrap();
    let out = w.send("series/v2", &[]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "version").as_deref(), Some("v2"));
    assert_eq!(value(&out, "local-ref").as_deref(), Some("topic/x/v2"));
    assert_eq!(value(&out, "range_diff").as_deref(), Some("topic/x/v1"));
    let cover = std::fs::read_to_string(value(&out, "patch.1").unwrap()).unwrap();
    assert!(cover.contains("Changes since v1:"), "{cover}");
    assert!(cover.contains("Range-diff against v1"), "{cover}");
}

#[test]
fn a_conflict_names_the_commit_and_leaves_nothing() {
    let w = world();
    git(&w.repo, &["checkout", "-q", "for-next"]);
    commit(&w.repo, "x.txt", "x from the target\n", "x: the target's own x");
    git(&w.repo, &["checkout", "-q", "main"]);
    let out = w.send("series/v1", &[]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let first = git(&w.repo, &["rev-parse", "topic/x~1"]);
    assert_eq!(value(&out, "conflict.commit").as_deref(), Some(first.as_str()));
    assert_eq!(value(&out, "conflict.subject").as_deref(), Some("x: add x"));
    assert_eq!(value(&out, "conflict.path.1").as_deref(), Some("x.txt"));
    assert_eq!(value(&out, "local-ref"), None);
    assert_eq!(git(&w.repo, &["branch", "--list", "send/*"]), "");
    assert_eq!(git(&w.repo, &["tag", "-l", "topic/*"]), "");
    assert!(!w.root.join("worktrees/app/rhizoid/send-x-v1").exists());
}

#[test]
fn a_failed_check_cuts_the_series_but_tags_nothing() {
    let w = world();
    std::fs::write(&w.config, "[[send.check]]\nname = \"style\"\nrun = [\"sh\", \"-c\", \"echo 'WARNING: a finding'; exit 3\"]\n").unwrap();
    let out = w.send("series/v1", &[]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(value(&out, "check.1.exit").as_deref(), Some("3"));
    assert_eq!(value(&out, "local-ref"), None);
    assert_eq!(
        git(&w.repo, &["branch", "--list", "send/*"]),
        "",
        "nothing kept but the series to read"
    );
    let notes = std::fs::read_to_string(w.root.join("series/v1/SEND-NOTES")).unwrap();
    assert!(
        notes.contains("== check style (exit 3)") && notes.contains("WARNING: a finding"),
        "{notes}"
    );
    std::fs::write(&w.config, "").unwrap();
    let again = w.send("series/v1-again", &[]);
    assert_eq!(value(&again, "version").as_deref(), Some("v1"), "the version was not used up");
}

#[test]
fn what_cannot_be_trusted_is_refused_before_anything_is_made() {
    let w = world();
    for (args, why) in [
        (vec!["--tested", "HEAD"], "is not a commit's sha"),
        (vec!["--tested", "topic/x"], "is not a commit's sha"),
    ] {
        let out = w.send("series/v1", &args);
        assert_eq!(out.status.code(), Some(2), "{out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(why), "{out:?}");
    }
    std::fs::create_dir_all(w.root.join("full")).unwrap();
    std::fs::write(w.root.join("full/old.patch"), "x").unwrap();
    let out = w.send("full", &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("is not empty"), "{out:?}");
    let out = Command::new(bin())
        .args(["send-back", "topic/x", "--out", "o"])
        .arg("--repo")
        .arg(&w.repo)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("--onto"), "{out:?}");
    assert_eq!(git(&w.repo, &["branch", "--list", "send/*"]), "");
}

#[test]
fn without_a_config_the_names_are_the_defaults_and_there_are_no_checks() {
    let w = world();
    let out = Command::new(bin())
        .args(["send-back", "topic/x", "--onto", "for-next", "--json", "--out"])
        .arg(w.root.join("s"))
        .arg("--repo")
        .arg(&w.repo)
        .env("AGENT_WORKTREES", w.root.join("worktrees"))
        .env_remove("RHIZOID_CONFIG")
        .envs(IDENTITY)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["local-ref"], "topic/x/v1");
    assert_eq!(v["send_branch"], "send/x/v1");
    assert_eq!(v["checks"].as_array().unwrap().len(), 0);
}
