//! The real [`GitPort`]: one clone, driven with the git CLI (decision
//! `2026-11-23-standard-tools-drive-cut-or-write`) - its worktrees and its rerere memory are the ones a
//! person's git sees. Git runs with `LC_ALL=C` and never prompts.

use rhizoid_core::assemble::LOCK_REASON_PREFIX;
use rhizoid_core::{BranchSource, GitPort, MergeResult, PortError, Worktree};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// One clone, at `repo`.
pub struct GitCli {
    repo: PathBuf,
}

impl GitCli {
    pub fn new(repo: &Path) -> Self {
        GitCli { repo: repo.to_path_buf() }
    }

    /// git in the clone; its output, whatever its status.
    fn run(&self, dir: &Path, args: &[&str]) -> Result<Output, PortError> {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| PortError(format!("cannot run git: {e}")))
    }

    /// git in `dir`, which must succeed; its stdout.
    fn ok_in(&self, dir: &Path, args: &[&str]) -> Result<String, PortError> {
        let out = self.run(dir, args)?;
        if !out.status.success() {
            return Err(PortError(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn ok(&self, args: &[&str]) -> Result<String, PortError> {
        self.ok_in(&self.repo, args)
    }

    /// The commit `rev` names, in `dir`, or `None`.
    fn verify(&self, dir: &Path, rev: &str) -> Result<Option<String>, PortError> {
        let out = self.run(dir, &["rev-parse", "-q", "--verify", &format!("{rev}^{{commit}}")])?;
        Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }
}

fn lines(s: &str) -> Vec<String> {
    s.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
}

fn path_arg(p: &Path) -> Result<&str, PortError> {
    p.to_str().ok_or_else(|| PortError(format!("{} is not UTF-8", p.display())))
}

impl GitPort for GitCli {
    fn fetch_prune(&self) -> Result<(), PortError> {
        self.ok(&["fetch", "-q", "--all", "--prune", "--no-prune-tags"]).map(drop)
    }

    fn commit(&self, rev: &str) -> Result<Option<String>, PortError> {
        self.verify(&self.repo, rev)
    }

    fn branch(&self, name: &str) -> Result<Option<(String, BranchSource)>, PortError> {
        if let Some(sha) = self.verify(&self.repo, &format!("refs/heads/{name}"))? {
            return Ok(Some((sha, BranchSource::Local)));
        }
        Ok(self
            .verify(&self.repo, &format!("refs/remotes/origin/{name}"))?
            .map(|sha| (sha, BranchSource::Origin)))
    }

    fn tag_object(&self, name: &str) -> Result<(String, String), PortError> {
        tag_object(&self.repo, name).map_err(PortError)
    }

    fn worktrees(&self) -> Result<Vec<Worktree>, PortError> {
        worktrees(&self.repo).map_err(PortError)
    }

    fn remove_worktree(&self, path: &Path, force: bool) -> Result<(), PortError> {
        let path = path_arg(path)?;
        self.ok(&["worktree", "unlock", path])?;
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(path);
        self.ok(&args).map(drop)
    }

    fn tags(&self, prefix: &str) -> Result<Vec<String>, PortError> {
        self.ok(&["tag", "-l", &format!("{prefix}*")]).map(|s| lines(&s))
    }

    fn share_rerere(&self) -> Result<(), PortError> {
        self.ok(&["config", "rerere.enabled", "true"]).map(drop)
    }

    fn add_worktree(&self, path: &Path, commit: &str) -> Result<(), PortError> {
        self.ok(&["worktree", "add", "-q", "--detach", path_arg(path)?, commit]).map(drop)
    }

    fn lock_worktree(&self, path: &Path, reason: &str) -> Result<(), PortError> {
        self.ok(&["worktree", "lock", "--reason", reason, path_arg(path)?]).map(drop)
    }

    fn merge(&self, path: &Path, commit: &str, message: &str) -> Result<MergeResult, PortError> {
        let rerere = ["-c", "rerere.enabled=true", "-c", "rerere.autoUpdate=true"];
        let before = self.head(path)?;
        let mut args = rerere.to_vec();
        args.extend(["merge", "-q", "--no-ff", "--no-edit", "-m", message, commit]);
        let merged = self.run(path, &args)?;
        if merged.status.success() {
            return Ok(if self.head(path)? == before {
                MergeResult::AlreadyContained
            } else {
                MergeResult::Merged { by_rerere: false }
            });
        }
        let why = String::from_utf8_lossy(&merged.stderr).trim().to_string();
        if self.verify(path, "MERGE_HEAD")?.is_none() {
            return Err(PortError(format!(
                "git merge {commit} failed before merging, in the worktree {}: {why}",
                path.display()
            )));
        }
        let unmerged =
            nul_separated(&self.ok_in(path, &["diff", "-z", "--name-only", "--diff-filter=U"])?);
        if !unmerged.is_empty() {
            return Ok(MergeResult::Conflict { paths: unmerged });
        }
        if !why.contains("using previous resolution") {
            return Err(PortError(format!(
                "git merge {commit} stopped with nothing in conflict, in the worktree {}: {why}",
                path.display()
            )));
        }
        let mut commit_args = rerere.to_vec();
        commit_args.extend(["commit", "-q", "--no-edit", "--cleanup=strip"]);
        self.ok_in(path, &commit_args)?;
        Ok(MergeResult::Merged { by_rerere: true })
    }

    fn changed_paths(&self, base: &str, commit: &str) -> Result<Vec<String>, PortError> {
        let range = format!("{base}...{commit}");
        self.ok(&["diff", "-z", "--no-renames", "--name-only", &range]).map(|s| nul_separated(&s))
    }

    fn head(&self, path: &Path) -> Result<String, PortError> {
        self.verify(path, "HEAD")?
            .ok_or_else(|| PortError(format!("the worktree {} has no HEAD", path.display())))
    }

    fn tag(&self, name: &str, commit: &str, message: &str) -> Result<String, PortError> {
        self.ok(&["tag", "-a", "-m", message, name, commit])?;
        Ok(self.ok(&["rev-parse", &format!("refs/tags/{name}")])?.trim().to_string())
    }

    fn tree_diff(&self, a: &str, b: &str) -> Result<Vec<String>, PortError> {
        let tree = |r: &str| self.ok(&["rev-parse", &format!("{r}^{{tree}}")]);
        if tree(a)?.trim() == tree(b)?.trim() {
            return Ok(Vec::new());
        }
        self.ok(&["diff", "-z", "--no-renames", "--name-only", a, b]).map(|s| nul_separated(&s))
    }
}

/// Paths git printed with `-z`: as they are, never quoted.
fn nul_separated(s: &str) -> Vec<String> {
    s.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect()
}

impl GitCli {
    /// The main clone's folder (for a linked worktree, the clone it belongs to), as a real path.
    pub fn main_clone(&self) -> Result<PathBuf, PortError> {
        let common = self.ok(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
        let common = std::fs::canonicalize(common.trim()).map_err(|e| PortError(e.to_string()))?;
        match common.file_name().and_then(|n| n.to_str()) {
            Some(".git") => Ok(common.parent().map(Path::to_path_buf).unwrap_or(common)),
            _ => Ok(common),
        }
    }
}

/// git in `dir`, which must succeed; its stdout, or what git said.
fn git_in(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A tag's object sha and its message: empty unless it is an annotated tag (git would give a lightweight tag's
/// commit message, which is no tag's recipe).
pub fn tag_object(clone: &Path, name: &str) -> Result<(String, String), String> {
    let format = "--format=%(objecttype)%00%(objectname)%00%(contents)";
    let out = git_in(clone, &["for-each-ref", format, &format!("refs/tags/{name}")])?;
    let mut f = out.splitn(3, '\0');
    match (f.next(), f.next(), f.next()) {
        (Some("tag"), Some(sha), Some(message)) => Ok((sha.to_string(), message.to_string())),
        (Some(_), Some(sha), _) => Ok((sha.to_string(), String::new())),
        _ => Err(format!("no tag {name}")),
    }
}

/// A clone's worktrees (git 2.36 or later, for `-z`). Only the ones assemble locked are read for changes, with
/// `--no-optional-locks`, so reading never takes a lock a running merge needs.
pub fn worktrees(clone: &Path) -> Result<Vec<Worktree>, String> {
    let list = git_in(clone, &["worktree", "list", "--porcelain", "-z"])?;
    let mut out = Vec::new();
    let mut current: Option<Worktree> = None;
    for field in list.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            out.extend(current.take());
            current = Some(Worktree {
                path: path.to_string(),
                head: None,
                lock_reason: None,
                clean: None,
            });
        } else if let Some(w) = current.as_mut() {
            if let Some(head) = field.strip_prefix("HEAD ") {
                w.head = Some(head.to_string());
            } else if let Some(reason) = field.strip_prefix("locked ") {
                w.lock_reason = Some(reason.to_string());
            }
        }
    }
    out.extend(current);
    for w in out
        .iter_mut()
        .filter(|w| w.lock_reason.as_deref().is_some_and(|r| r.starts_with(LOCK_REASON_PREFIX)))
    {
        w.clean = git_in(Path::new(&w.path), &["--no-optional-locks", "status", "--porcelain"])
            .ok()
            .map(|s| s.is_empty());
    }
    Ok(out)
}
