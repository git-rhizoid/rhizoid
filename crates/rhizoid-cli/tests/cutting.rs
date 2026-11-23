//! The cutting verbs against a fake gh on PATH (it answers from files and logs every call) and local bare
//! repositories standing in for the fork and its upstream: no test touches GitHub.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-cutting-test-{}-{}",
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

/// gh, faked: repositories are files `OWNER_REPO.default` (its default branch) and `OWNER_REPO.parent`.
const FAKE_GH: &str = r#"#!/bin/sh
echo "$*" >> "$FAKE_GH/calls"
key() { echo "$1" | tr / _; }
case "$1" in
api)
  if [ "$2" = user ]; then echo me; exit 0; fi
  if [ "$2" = -X ]; then echo none; exit 0; fi
  f="$FAKE_GH/$(key "${2#repos/}")"
  case "$4" in
    .default_branch) cat "$f.default" ;;
    *parent*) cat "$f.parent" 2>/dev/null; true ;;
    *) echo deadbeef ;;
  esac ;;
repo)
  case "$2" in
    view) test -f "$FAKE_GH/$(key "$3").default" ;;
    fork)
      org=$(echo "$*" | sed -n 's/.*--org \([^ ]*\).*/\1/p'); [ -n "$org" ] || org=me
      cp "$FAKE_GH/$(key "$3").default" "$FAKE_GH/${org}_${3##*/}.default"
      echo "$3" > "$FAKE_GH/${org}_${3##*/}.parent" ;;
  esac ;;
esac
"#;

struct World {
    root: PathBuf,
    fake: PathBuf,
    upstream: PathBuf,
    fork: PathBuf,
    clone: PathBuf,
    graph: PathBuf,
}

/// An upstream (bare), the fork (a bare copy of it), the fork's clone at ROOT/acme/fork, and a fake gh that
/// knows acme/fork (default master, parent `parent`) and up/stream.
fn world(parent: Option<&str>) -> World {
    let root = tempdir();
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    git(&seed, &["init", "-q", "-b", "master"]);
    std::fs::write(seed.join("README"), "upstream\n").unwrap();
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "first"]);
    let upstream = root.join("bare/upstream.git");
    let fork = root.join("bare/fork.git");
    for bare in [&upstream, &fork] {
        let ok = Command::new("git")
            .args(["clone", "-q", "--bare"])
            .arg(&seed)
            .arg(bare)
            .status()
            .unwrap();
        assert!(ok.success());
    }
    let clone = root.join("acme/fork");
    let ok = Command::new("git").args(["clone", "-q"]).arg(&fork).arg(&clone).status().unwrap();
    assert!(ok.success());
    // the clone names the fork by its GitHub URL, as a real one does; git is sent to the bare repository instead
    git(&clone, &["remote", "set-url", "origin", "https://github.com/acme/fork"]);
    git(
        &clone,
        &["config", &format!("url.{}.insteadOf", fork.display()), "https://github.com/acme/fork"],
    );
    let fake = root.join("fakegh");
    std::fs::create_dir_all(fake.join("bin")).unwrap();
    std::fs::write(fake.join("bin/gh"), FAKE_GH).unwrap();
    assert!(Command::new("chmod").arg("+x").arg(fake.join("bin/gh")).status().unwrap().success());
    std::fs::write(fake.join("acme_fork.default"), "master\n").unwrap();
    std::fs::write(fake.join("up_stream.default"), "master\n").unwrap();
    if let Some(p) = parent {
        std::fs::write(fake.join("acme_fork.parent"), format!("{p}\n")).unwrap();
    }
    let graph = root.join("rhizoid.graph.toml");
    World { root, fake, upstream, fork, clone, graph }
}

impl World {
    fn graph(&self, text: &str) {
        std::fs::write(&self.graph, text).unwrap();
    }

    fn rhizoid(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.fake.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(bin())
            .args(args)
            .arg("--graph")
            .arg(&self.graph)
            .arg("--root")
            .arg(&self.root)
            .env("PATH", path)
            .env("FAKE_GH", &self.fake)
            .env("AGENT_WORKTREES", self.root.join("worktrees"))
            .envs(IDENTITY)
            .output()
            .unwrap()
    }

    /// The gh calls made, one per line.
    fn calls(&self) -> String {
        std::fs::read_to_string(self.fake.join("calls")).unwrap_or_default()
    }

    fn graph_text(&self) -> String {
        std::fs::read_to_string(&self.graph).unwrap()
    }
}

fn value(out: &Output, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix(&prefix).map(str::to_string))
}

/// No gh call that writes: no fork, no POST.
fn wrote_to_github(calls: &str) -> bool {
    calls.lines().any(|c| c.starts_with("repo fork") || c.contains("-X POST"))
}

const LINUX_LIKE: &str = "[[fork]]\nfork = \"acme/fork\"\nupstream = \"up/stream\"\n\n[[integration]]\nrepo = \"acme/fork\"\ntopics = [\"topic/ITEM\"]\nreleases = \"torvalds/linux\"\n";

#[test]
fn importing_with_an_integration_patch_side_records_it_and_writes_nothing_outward() {
    let w = world(Some("up/stream"));
    w.graph(LINUX_LIKE);
    let plan = w.rhizoid(&["import", "acme/fork", "--patches", "integration"]);
    assert_eq!(plan.status.code(), Some(0), "{plan:?}");
    assert!(value(&plan, "step.1").unwrap().starts_with("record in the graph"), "{plan:?}");
    assert_eq!(value(&plan, "step.1.outward").as_deref(), Some("no"));
    assert_eq!(value(&plan, "step.2"), None, "no patch branch, nothing pushed");
    assert_eq!(w.graph_text(), LINUX_LIKE, "a plan changes nothing");

    let done = w.rhizoid(&["import", "acme/fork", "now", "--patches", "integration"]);
    assert_eq!(done.status.code(), Some(0), "{done:?}");
    let text = w.graph_text();
    assert!(text.contains("fork = \"acme/fork\"\ntracks = \"torvalds/linux\"\npatches = \"integration\"\nadded = \"import\"\nupstream = \"up/stream\""), "{text}");
    assert!(text.contains("[[integration]]\nrepo = \"acme/fork\""), "the rest kept");
    assert!(!wrote_to_github(&w.calls()), "{}", w.calls());
    assert_eq!(git(&w.fork, &["branch", "--list"]), "* master", "nothing pushed to the fork");
}

#[test]
fn an_integration_patch_side_needs_its_edge() {
    let w = world(Some("up/stream"));
    let graph = "[[fork]]\nfork = \"acme/fork\"\nupstream = \"up/stream\"\n";
    w.graph(graph);
    let plan = w.rhizoid(&["import", "acme/fork", "--patches", "integration"]);
    assert_eq!(plan.status.code(), Some(1), "{plan:?}");
    assert!(
        value(&plan, "blocked.1").unwrap().contains("no [[integration]] edge names acme/fork"),
        "{plan:?}"
    );
    let refused = w.rhizoid(&["import", "acme/fork", "now", "--patches", "integration"]);
    assert_eq!(refused.status.code(), Some(1), "{refused:?}");
    assert_eq!(w.graph_text(), graph);
}

#[test]
fn importing_with_a_patch_branch_makes_it_from_tracking_with_its_module_file_and_pushes_it() {
    let w = world(None);
    w.graph(&format!("[[fork]]\nfork = \"acme/fork\"\nupstream = \"{}\"\n", w.upstream.display()));
    let plan = w.rhizoid(&["import", "acme/fork"]);
    assert_eq!(plan.status.code(), Some(0), "{plan:?}");
    assert!(
        value(&plan, "step.1")
            .unwrap()
            .starts_with("make rhizoid/patch from the fork's master (origin)"),
        "{plan:?}"
    );
    assert_eq!(value(&plan, "step.2").as_deref(), Some("push rhizoid/patch to the fork (origin)"));
    assert_eq!(value(&plan, "step.2.outward").as_deref(), Some("yes"));
    assert!(value(&plan, "note.1").unwrap().contains("no GitHub fork"), "{plan:?}");
    assert_eq!(git(&w.fork, &["branch", "--list", "rhizoid/*"]), "", "a plan pushes nothing");

    let done = w.rhizoid(&["import", "acme/fork", "now"]);
    assert_eq!(done.status.code(), Some(0), "{done:?}");
    let module = git(&w.fork, &["show", "rhizoid/patch:.rhizoid/module.toml"]);
    assert!(
        module.contains(&format!("origin = \"{}\"", w.upstream.display()))
            && module.contains("added_as = \"imported\""),
        "{module}"
    );
    assert_eq!(
        git(&w.fork, &["rev-parse", "rhizoid/patch~1"]),
        git(&w.fork, &["rev-parse", "master"]),
        "made from tracking"
    );
    assert!(
        w.graph_text().contains("patches = \"rhizoid/patch\"\nadded = \"import\""),
        "{}",
        w.graph_text()
    );
    assert!(!wrote_to_github(&w.calls()));
}

#[test]
fn update_fast_forwards_tracking_from_a_non_github_upstream_and_merges_it_into_the_patch_branch() {
    let w = world(None);
    w.graph(&format!("[[fork]]\nfork = \"acme/fork\"\nupstream = \"{}\"\n", w.upstream.display()));
    assert_eq!(w.rhizoid(&["import", "acme/fork", "now"]).status.code(), Some(0));
    let work = w.root.join("upwork");
    assert!(Command::new("git")
        .args(["clone", "-q"])
        .arg(&w.upstream)
        .arg(&work)
        .status()
        .unwrap()
        .success());
    std::fs::write(work.join("NEW"), "upstream moved\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "upstream moves"]);
    git(&work, &["push", "-q", "origin", "master"]);

    let plan = w.rhizoid(&["update", "acme/fork"]);
    assert_eq!(plan.status.code(), Some(0), "{plan:?}");
    assert!(value(&plan, "step.1").unwrap().starts_with("fetch "), "{plan:?}");
    assert!(
        value(&plan, "step.2").unwrap().starts_with("merge master into rhizoid/patch"),
        "{plan:?}"
    );
    let done = w.rhizoid(&["update", "acme/fork", "now"]);
    assert_eq!(done.status.code(), Some(0), "{done:?}");
    assert_eq!(
        git(&w.fork, &["rev-parse", "master"]),
        git(&w.upstream, &["rev-parse", "master"]),
        "tracking fast-forwarded"
    );
    git(&w.fork, &["merge-base", "--is-ancestor", "master", "rhizoid/patch"]);
    assert!(
        git(&w.fork, &["show", "rhizoid/patch:.rhizoid/module.toml"]).contains("added_as"),
        "the patch side kept"
    );
}

#[test]
fn update_refuses_a_tracking_branch_changed_by_hand() {
    let w = world(None);
    w.graph(&format!("[[fork]]\nfork = \"acme/fork\"\nupstream = \"{}\"\n", w.upstream.display()));
    assert_eq!(w.rhizoid(&["import", "acme/fork", "now"]).status.code(), Some(0));
    std::fs::write(w.clone.join("HAND"), "by hand\n").unwrap();
    git(&w.clone, &["add", "-A"]);
    git(&w.clone, &["commit", "-q", "-m", "on tracking by hand"]);
    git(&w.clone, &["push", "-q", "origin", "master"]);
    let before = git(&w.fork, &["rev-parse", "master"]);
    let out = w.rhizoid(&["update", "acme/fork", "now"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(value(&out, "error").unwrap().contains("changed by hand"), "{out:?}");
    assert_eq!(git(&w.fork, &["rev-parse", "master"]), before, "nothing pushed");
}

#[test]
fn remove_takes_the_cutting_out_of_the_graph_and_never_the_fork() {
    let w = world(Some("up/stream"));
    w.graph(LINUX_LIKE);
    let out = w.rhizoid(&["remove", "acme/fork", "now"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(!w.graph_text().contains("[[fork]]"), "{}", w.graph_text());
    assert!(w.graph_text().contains("[[integration]]"));
    assert!(w.calls().is_empty(), "GitHub is not asked: {}", w.calls());
}

#[test]
fn add_forks_on_github_only_with_now() {
    let w = world(Some("up/stream"));
    w.graph("fork_org = \"acme2\"\n\n[[integration]]\nrepo = \"acme2/stream\"\n");
    let plan = w.rhizoid(&["add", "up/stream", "--patches", "integration"]);
    assert_eq!(plan.status.code(), Some(0), "{plan:?}");
    assert_eq!(value(&plan, "step.1").as_deref(), Some("fork up/stream into acme2 on GitHub"));
    assert_eq!(value(&plan, "step.1.outward").as_deref(), Some("yes"));
    assert!(!wrote_to_github(&w.calls()), "a plan forks nothing");
    let done = w.rhizoid(&["add", "up/stream", "now", "--patches", "integration"]);
    assert_eq!(done.status.code(), Some(0), "{done:?}");
    assert!(
        w.calls()
            .lines()
            .any(|c| c.starts_with("repo fork up/stream") && c.contains("--org acme2")),
        "{}",
        w.calls()
    );
    assert!(
        w.graph_text()
            .contains("fork = \"acme2/stream\"\nbranch = \"master\"\nupstream = \"up/stream\""),
        "{}",
        w.graph_text()
    );
    assert!(w.graph_text().contains("added = \"add\""));
}

#[test]
fn refresh_reads_each_fork_and_changes_nothing() {
    let w = world(Some("someone/else"));
    w.graph(LINUX_LIKE);
    let out = w.rhizoid(&["refresh"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(
        value(&out, "cutting.1.says").unwrap().contains("GitHub's parent is someone/else"),
        "{out:?}"
    );
    assert_eq!(w.graph_text(), LINUX_LIKE);
}

#[test]
fn a_clone_whose_origin_is_the_upstream_pushes_to_the_fork_remote_not_origin() {
    let w = world(None);
    // as the real sxmo-utils clone is: origin the upstream, the GitHub fork another remote
    git(&w.clone, &["remote", "rename", "origin", "mirror"]);
    git(&w.clone, &["remote", "add", "origin", w.upstream.to_str().unwrap()]);
    git(&w.clone, &["fetch", "-q", "mirror"]);
    w.graph(&format!("[[fork]]\nfork = \"acme/fork\"\nupstream = \"{}\"\n", w.upstream.display()));
    let plan = w.rhizoid(&["import", "acme/fork"]);
    assert_eq!(plan.status.code(), Some(0), "{plan:?}");
    assert_eq!(value(&plan, "step.2").as_deref(), Some("push rhizoid/patch to the fork (mirror)"));
    assert_eq!(w.rhizoid(&["import", "acme/fork", "now"]).status.code(), Some(0));
    assert!(
        git(&w.fork, &["branch", "--list", "rhizoid/*"]).contains("rhizoid/patch"),
        "pushed to the fork"
    );
    assert_eq!(
        git(&w.upstream, &["branch", "--list", "rhizoid/*"]),
        "",
        "nothing pushed to the upstream"
    );
}

#[test]
fn a_clone_with_no_remote_for_the_fork_is_said() {
    let w = world(None);
    git(&w.clone, &["remote", "set-url", "origin", w.upstream.to_str().unwrap()]);
    w.graph(&format!("[[fork]]\nfork = \"acme/fork\"\nupstream = \"{}\"\n", w.upstream.display()));
    let plan = w.rhizoid(&["import", "acme/fork"]);
    assert_eq!(plan.status.code(), Some(1), "{plan:?}");
    assert!(
        value(&plan, "blocked.1").unwrap().contains("no remote for github.com/acme/fork"),
        "{plan:?}"
    );
}

#[test]
fn a_record_that_conflicts_with_the_graph_blocks_the_plan_before_anything_outward() {
    let w = world(Some("up/stream"));
    w.graph("[[fork]]\nfork = \"acme/fork\"\nupstream = \"up/stream\"\nbranch = \"main\"\n\n[[integration]]\nrepo = \"acme/fork\"\n");
    let plan =
        w.rhizoid(&["import", "acme/fork", "--patches", "integration", "--tracked-ref", "master"]);
    assert_eq!(plan.status.code(), Some(1), "{plan:?}");
    assert!(value(&plan, "blocked.1").unwrap().contains("change it by hand"), "{plan:?}");
}

#[test]
fn an_integration_cutting_tracks_its_releases_source_unless_a_branch_is_named() {
    let w = world(Some("up/stream"));
    w.graph("[[fork]]\nfork = \"acme/fork\"\nupstream = \"up/stream\"\n\n[[integration]]\nrepo = \"acme/fork\"\n");
    let plan = w.rhizoid(&["import", "acme/fork", "--patches", "integration"]);
    assert_eq!(plan.status.code(), Some(1), "{plan:?}");
    assert!(value(&plan, "blocked.1").unwrap().contains("names no releases source"), "{plan:?}");
    let named = w.rhizoid(&[
        "import",
        "acme/fork",
        "now",
        "--patches",
        "integration",
        "--tracked-ref",
        "master",
    ]);
    assert_eq!(named.status.code(), Some(0), "{named:?}");
    assert!(
        w.graph_text().contains("branch = \"master\""),
        "--tracked-ref wins: {}",
        w.graph_text()
    );
    assert!(!w.graph_text().contains("tracks ="));
}

#[test]
fn update_of_an_integration_cutting_pushes_nothing_and_names_topics_behind_the_newest_release() {
    let w = world(Some("up/stream"));
    git(&w.fork, &["tag", "v1.0", "master"]);
    git(&w.clone, &["fetch", "-q", "--tags"]);
    git(&w.clone, &["branch", "-q", "topic/a", "v1.0"]);
    let work = w.root.join("upwork");
    assert!(Command::new("git")
        .args(["clone", "-q"])
        .arg(&w.upstream)
        .arg(&work)
        .status()
        .unwrap()
        .success());
    std::fs::write(work.join("NEW"), "1.1\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "1.1"]);
    git(&work, &["tag", "-a", "-m", "v1.1", "v1.1"]);
    git(&work, &["push", "-q", "origin", "master", "v1.1"]);
    git(&w.clone, &["fetch", "-q", w.upstream.to_str().unwrap(), "refs/tags/v1.1:refs/tags/v1.1"]);
    w.graph(&format!(
        "[clones]\n\"acme/fork\" = \"acme/fork\"\n\n[[fork]]\nfork = \"acme/fork\"\nupstream = \"up/stream\"\ntracks = \"{u}\"\npatches = \"integration\"\nadded = \"import\"\n\n[[integration]]\nrepo = \"acme/fork\"\ntopics = [\"topic/ITEM\"]\nreleases = \"{u}\"\n",
        u = w.upstream.display()
    ));
    let before = git(&w.fork, &["rev-parse", "master"]);
    let out = w.rhizoid(&["update", "acme/fork", "now"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out, "step.1"), None, "no step: nothing to merge or push");
    assert!(value(&out, "note.1").unwrap().contains("nothing is merged or pushed"), "{out:?}");
    assert!(
        value(&out, "note.2").unwrap().contains("topic/a is on v1.0; the newest release is v1.1"),
        "{out:?}"
    );
    assert_eq!(git(&w.fork, &["rev-parse", "master"]), before);
    assert!(!wrote_to_github(&w.calls()), "{}", w.calls());
}
