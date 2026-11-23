//! The real [`Repos`]: clones read with `git`, remotes asked with `git ls-remote`, the host asked with `gh`
//! (the `auth-via-gh` decision). Everything it does is a read; `--fetch` only brings a clone's refs up to date.
//! Git runs with `LC_ALL=C`, so what it says on stderr is what this reads it as.

use rhizoid_core::graph::{Remote, RepoName};
use rhizoid_core::{Graph, KernelReads, Level, RepoError, Repos, Worktree};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct GitRepos<'a> {
    root: PathBuf,
    graph: &'a Graph,
    fetch: bool,
    fetched: RefCell<BTreeSet<RepoName>>,
}

impl<'a> GitRepos<'a> {
    pub fn new(root: &Path, graph: &'a Graph, fetch: bool) -> Self {
        GitRepos { root: root.to_path_buf(), graph, fetch, fetched: RefCell::default() }
    }

    fn clone_dir(&self, repo: &RepoName) -> Result<PathBuf, RepoError> {
        let dir = self.root.join(self.graph.clone_path(repo));
        if !dir.is_dir() {
            return Err(RepoError::NoClone(dir.display().to_string()));
        }
        if self.fetch && self.fetched.borrow_mut().insert(repo.clone()) {
            // Branches pruned, then tags fetched without pruning: a tag made here (an int tag) stays.
            for args in [
                ["fetch", "-q", "--prune", "--no-prune-tags", "origin"],
                ["fetch", "-q", "--tags", "--no-prune-tags", "origin"],
            ] {
                let out = git(&dir, &args)?;
                if !out.status.success() {
                    return Err(RepoError::Failed(format!(
                        "{repo}: git fetch failed: {}",
                        stderr(&out)
                    )));
                }
            }
        }
        Ok(dir)
    }
}

fn plain(command: &mut Command) -> Result<Output, RepoError> {
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| RepoError::Failed(format!("cannot run {:?}: {e}", command.get_program())))
}

fn git(dir: &Path, args: &[&str]) -> Result<Output, RepoError> {
    plain(Command::new("git").arg("-C").arg(dir).args(args))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}

impl Repos for GitRepos<'_> {
    fn file(&self, repo: &RepoName, rev: &str, path: &str) -> Result<Option<String>, RepoError> {
        let dir = self.clone_dir(repo)?;
        let known = git(&dir, &["rev-parse", "-q", "--verify", &format!("{rev}^{{commit}}")])?;
        if !known.status.success() {
            return Err(RepoError::NoRev { repo: repo.to_string(), rev: rev.to_string() });
        }
        let out = git(&dir, &["show", &format!("{rev}:{path}")])?;
        if out.status.success() {
            return String::from_utf8(out.stdout).map(Some).map_err(|_| {
                RepoError::Failed(format!("{repo}: {path} at {rev} is not UTF-8 text"))
            });
        }
        let why = stderr(&out);
        if why.contains("does not exist") || why.contains("exists on disk, but not in") {
            Ok(None)
        } else {
            Err(RepoError::Failed(format!("{repo}: git show {rev}:{path}: {why}")))
        }
    }

    fn tags(&self, repo: &RepoName) -> Result<Vec<String>, RepoError> {
        let dir = self.clone_dir(repo)?;
        let out = git(&dir, &["tag", "-l"])?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!("{repo}: git tag: {}", stderr(&out))));
        }
        Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect())
    }

    fn head(&self, remote: &Remote, branch: Option<&str>) -> Result<String, RepoError> {
        let reference = branch.map_or("HEAD".to_string(), |b| format!("refs/heads/{b}"));
        let out = plain(Command::new("git").args(["ls-remote", "--", &remote.url(), &reference]))?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!("{remote}: git ls-remote: {}", stderr(&out))));
        }
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .map(str::to_string)
            .ok_or_else(|| RepoError::Failed(format!("{remote} has no {reference}")))
    }

    fn level(&self, fork: &Remote, fork_head: &str, upstream_head: &str) -> Level {
        let Remote::GitHub(repo) = fork else {
            return Level::Unknown(format!("{fork} is not on GitHub"));
        };
        let compare = format!("repos/{repo}/compare/{fork_head}...{upstream_head}");
        let jq = "[.status, .ahead_by, .behind_by] | @tsv";
        match Command::new("gh").args(["api", &compare, "--jq", jq]).stdin(Stdio::null()).output() {
            Ok(o) if o.status.success() => parse_compare(&String::from_utf8_lossy(&o.stdout)),
            Ok(o) => Level::Unknown(format!("gh api: {}", stderr(&o))),
            Err(e) => Level::Unknown(format!("cannot run gh: {e}")),
        }
    }
}

impl GitRepos<'_> {
    /// git in the clone of `repo`, which must succeed; its stdout.
    fn read(&self, repo: &RepoName, args: &[&str]) -> Result<String, RepoError> {
        let out = git(&self.clone_dir(repo)?, args)?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!(
                "{repo}: git {}: {}",
                args.join(" "),
                stderr(&out)
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl KernelReads for GitRepos<'_> {
    fn refs(&self, repo: &RepoName, namespace: &str) -> Result<Vec<(String, String)>, RepoError> {
        let format = "--format=%(refname)%00%(objectname)%00%(*objectname)";
        let out = self.read(repo, &["for-each-ref", format, namespace])?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\0');
                let (name, object, peeled) = (f.next()?, f.next()?, f.next().unwrap_or(""));
                let commit = if peeled.is_empty() { object } else { peeled };
                Some((name.strip_prefix(namespace)?.to_string(), commit.to_string()))
            })
            .collect())
    }

    fn is_ancestor(&self, repo: &RepoName, a: &str, b: &str) -> Result<bool, RepoError> {
        let out = git(&self.clone_dir(repo)?, &["merge-base", "--is-ancestor", a, b])?;
        match out.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => {
                Err(RepoError::Failed(format!("{repo}: git merge-base {a} {b}: {}", stderr(&out))))
            }
        }
    }

    fn count(&self, repo: &RepoName, from: &str, to: &str) -> Result<u64, RepoError> {
        let n = self.read(repo, &["rev-list", "--count", &format!("{from}..{to}")])?;
        n.trim()
            .parse()
            .map_err(|_| RepoError::Failed(format!("{repo}: git rev-list --count said {n}")))
    }

    fn nearest_release(
        &self,
        repo: &RepoName,
        rev: &str,
        prefix: &str,
    ) -> Result<Option<String>, RepoError> {
        let pattern = format!("{prefix}[0-9]*");
        let out = git(
            &self.clone_dir(repo)?,
            &["describe", "--tags", "--abbrev=0", "--match", &pattern, rev],
        )?;
        Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }

    fn tag_message(&self, repo: &RepoName, tag: &str) -> Result<String, RepoError> {
        crate::git_cli::tag_object(&self.clone_dir(repo)?, tag)
            .map(|(_, message)| message)
            .map_err(|e| RepoError::Failed(format!("{repo}: {e}")))
    }

    fn tree_equal(&self, repo: &RepoName, a: &str, b: &str) -> Result<bool, RepoError> {
        let tree = |r: &str| self.read(repo, &["rev-parse", &format!("{r}^{{tree}}")]);
        Ok(tree(a)?.trim() == tree(b)?.trim())
    }

    fn worktrees(&self, repo: &RepoName) -> Result<Vec<Worktree>, RepoError> {
        crate::git_cli::worktrees(&self.clone_dir(repo)?)
            .map_err(|e| RepoError::Failed(format!("{repo}: {e}")))
    }
}

/// What `gh api compare FORK...UPSTREAM` printed (`status ahead_by behind_by`): `ahead` there is the
/// upstream ahead of the fork, so the fork is behind.
fn parse_compare(tsv: &str) -> Level {
    let unreadable = || Level::Unknown(format!("unreadable compare: {}", tsv.trim()));
    let mut f = tsv.split_whitespace();
    let (Some(status), Some(a), Some(b)) = (f.next(), f.next(), f.next()) else {
        return unreadable();
    };
    let (Ok(a), Ok(b)) = (a.parse::<u64>(), b.parse::<u64>()) else {
        return unreadable();
    };
    match status {
        "ahead" => Level::Behind(a),
        "behind" => Level::Ahead(b),
        "diverged" => Level::Diverged { behind: a, ahead: b },
        other => Level::Unknown(format!("the host says {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hosts_compare_is_read_from_the_forks_side() {
        assert_eq!(parse_compare("ahead\t4\t0\n"), Level::Behind(4));
        assert_eq!(parse_compare("behind\t0\t2\n"), Level::Ahead(2));
        assert_eq!(parse_compare("diverged\t3\t1\n"), Level::Diverged { behind: 3, ahead: 1 });
        assert!(matches!(parse_compare("identical\t0\t0"), Level::Unknown(_)));
        assert!(matches!(parse_compare(""), Level::Unknown(_)));
        assert!(matches!(parse_compare("ahead\tx\ty"), Level::Unknown(_)));
    }
}
