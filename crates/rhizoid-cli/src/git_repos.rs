//! The real [`Repos`]: clones read with `git`, remotes asked with `git ls-remote`, the host asked with `gh`
//! (the `auth-via-gh` decision). Everything it does is a read; `--fetch` only brings a clone's refs up to date.
//! Git runs with `LC_ALL=C`, so what it says on stderr is what this reads it as.

use rhizoid_core::graph::{Reference, Remote, RepoName};
use rhizoid_core::{
    Check, Finding, Graph, IntegrationReads, Level, RefRead, RepoError, Repos, Verdict, Worktree,
};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct GitRepos<'a> {
    root: PathBuf,
    graph: &'a Graph,
    fetch: bool,
    fetched: RefCell<BTreeSet<RepoName>>,
    moved: RefCell<Vec<Finding>>,
}

impl<'a> GitRepos<'a> {
    pub fn new(root: &Path, graph: &'a Graph, fetch: bool) -> Self {
        GitRepos {
            root: root.to_path_buf(),
            graph,
            fetch,
            fetched: RefCell::default(),
            moved: RefCell::default(),
        }
    }

    fn clone_dir(&self, repo: &RepoName) -> Result<PathBuf, RepoError> {
        let dir = self.root.join(self.graph.clone_path(repo));
        if !dir.is_dir() {
            return Err(RepoError::NoClone(dir.display().to_string()));
        }
        if self.fetch && self.fetched.borrow_mut().insert(repo.clone()) {
            self.fetch_clone(repo, &dir)?;
        }
        Ok(dir)
    }

    /// Branches pruned, then tags fetched without pruning and never forced: a tag made here (an int tag)
    /// stays, and a tag `origin` moved that the clone holds as it was is kept and becomes a finding - a
    /// silent clobber would hide the drift.
    fn fetch_clone(&self, repo: &RepoName, dir: &Path) -> Result<(), RepoError> {
        let branches = ["fetch", "--prune", "--no-prune-tags", "origin"];
        let out = git(dir, &branches)?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!("{repo}: {}", failure(&branches, &out))));
        }
        let tags = ["fetch", "--porcelain", "--tags", "--no-prune-tags", "origin"];
        let out = git(dir, &tags)?;
        let Some(moved) = fetched_tags(&out) else {
            return Err(RepoError::Failed(format!("{repo}: {}", failure(&tags, &out))));
        };
        self.moved.borrow_mut().extend(moved.into_iter().map(|(tag, here, theirs)| Finding {
            follower: repo.to_string(),
            source: "origin".to_string(),
            check: Check::TagOrigin,
            verdict: Verdict::MovedByHand,
            carried_by: None,
            detail: format!(
                "tag {tag} is {here} here and {theirs} on origin: kept as it is here, never clobbered (move one of them on purpose)"
            ),
        }));
        Ok(())
    }
}

/// The hits `git grep -n --null` printed at `rev`: `REV:PATH`, NUL, the line number, NUL, the line, newline. Read
/// field by field, so a path that holds a newline or a colon is still one path; the line itself holds no newline.
fn grep_hits(out: &str, rev: &str) -> Vec<(String, u64, String)> {
    let prefix = format!("{rev}:");
    let mut hits = Vec::new();
    let mut fields = out.split('\0');
    let mut head = fields.next().unwrap_or("").to_string();
    while let (Some(line), Some(rest)) = (fields.next(), fields.next()) {
        let (text, next) = rest.split_once('\n').unwrap_or((rest, ""));
        if let (Some(path), Ok(n)) = (head.strip_prefix(&prefix), line.parse()) {
            hits.push((path.to_string(), n, text.to_string()));
        }
        head = next.to_string();
    }
    hits
}

/// What a `git fetch --porcelain --tags` that may refuse tags ended with: the tags it refused (none when it
/// succeeded), or `None` when it failed for any other reason. git ends 1 for a refused tag and says nothing on
/// stderr; a refused ref that is not a tag, anything on stderr or any other end is a failure of its own.
fn fetched_tags(out: &Output) -> Option<Vec<(String, String, String)>> {
    if out.status.success() {
        return Some(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let moved = moved_tags(&stdout);
    let refused = stdout.lines().filter(|l| l.starts_with('!')).count();
    let only_moved_tags = !moved.is_empty() && moved.len() == refused;
    (out.status.code() == Some(1) && only_moved_tags && stderr(out).is_empty()).then_some(moved)
}

/// The tags `git fetch --porcelain` refused - one line `FLAG OLD NEW REF` per ref, `!` for a refused one -
/// each with the clone's object and `origin`'s; `None` when a refused ref is not a tag.
fn moved_tags(porcelain: &str) -> Vec<(String, String, String)> {
    porcelain
        .lines()
        .filter_map(|line| {
            let mut f = line.split(' ');
            let (flag, here, theirs, name) = (f.next()?, f.next()?, f.next()?, f.next()?);
            let tag = name.strip_prefix("refs/tags/")?;
            (flag == "!").then(|| (tag.to_string(), here.to_string(), theirs.to_string()))
        })
        .collect()
}

/// A git call that failed, in git's own words: the command and what it said, never an empty message.
pub(crate) fn failure(args: &[&str], out: &Output) -> String {
    let said = stderr(out);
    let said = if said.is_empty() {
        // some git commands say why on stdout (CONFLICT, nothing to commit, a refused ref)
        let code = out.status.code().map_or("a signal".to_string(), |c| format!("exit {c}"));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stdout: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
        if stdout.is_empty() {
            format!("ended with {code}, saying nothing")
        } else {
            format!("ended with {code}, saying on stdout: {}", stdout.join("; "))
        }
    } else {
        said
    };
    format!("git {}: {said}", args.join(" "))
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
            Err(RepoError::Failed(format!(
                "{repo}: {}",
                failure(&["show", &format!("{rev}:{path}")], &out)
            )))
        }
    }

    fn tags(&self, repo: &RepoName) -> Result<Vec<String>, RepoError> {
        let dir = self.clone_dir(repo)?;
        let out = git(&dir, &["tag", "-l"])?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!("{repo}: {}", failure(&["tag", "-l"], &out))));
        }
        Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect())
    }

    fn head(&self, remote: &Remote, branch: Option<&str>) -> Result<String, RepoError> {
        let reference = branch.map_or("HEAD".to_string(), |b| format!("refs/heads/{b}"));
        let out = plain(Command::new("git").args(["ls-remote", "--", &remote.url(), &reference]))?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!(
                "{remote}: {}",
                failure(&["ls-remote", "--", &remote.url(), &reference], &out)
            )));
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

    fn fetch_findings(&self) -> Vec<Finding> {
        self.moved.borrow().clone()
    }

    fn grep(
        &self,
        repo: &RepoName,
        rev: &str,
        text: &str,
    ) -> Result<Vec<(String, u64, String)>, RepoError> {
        // one record per hit: REV:PATH, NUL, LINE, NUL, the line; git ends 1 when nothing matches
        let args = ["grep", "-n", "-F", "-I", "--null", "-e", text, rev, "--"];
        let out = git(&self.clone_dir(repo)?, &args)?;
        match out.status.code() {
            Some(0) => {}
            Some(1) if stderr(&out).is_empty() => return Ok(Vec::new()),
            _ => return Err(RepoError::Failed(format!("{repo}: {}", failure(&args, &out)))),
        }
        Ok(grep_hits(&String::from_utf8_lossy(&out.stdout), rev))
    }

    fn reference(&self, r: &Reference) -> Result<RefRead, RepoError> {
        let dir = self.clone_dir(&r.repo)?;
        let verify = |rev: &str| -> Result<Option<String>, RepoError> {
            let out = git(&dir, &["rev-parse", "-q", "--verify", rev])?;
            Ok(out
                .status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
        };
        let tag = format!("refs/tags/{}", r.tag);
        if let Some(object) = verify(&tag)? {
            // a tag that is there but holds no commit is not missing: it holds something else
            return Ok(RefRead::At(
                verify(&format!("{tag}^{{commit}}"))?.unwrap_or(format!("{object} (no commit)")),
            ));
        }
        if !self.fetch {
            return Ok(RefRead::Missing);
        }
        // the ref into a ref of rhizoid's own, never FETCH_HEAD (another fetch may write it) and never the tag: the
        // tag is made only at the recorded commit, and only when absent
        let private = format!("refs/rhizoid/reference/{}", r.tag);
        let refspec = format!("+{}:{private}", r.reference);
        let mut args = vec!["fetch", "--no-tags", "--no-recurse-submodules"];
        let depth = r.depth.map(|d| d.to_string());
        if let Some(d) = &depth {
            args.extend(["--depth", d.as_str()]);
        }
        args.extend(["--", r.url.as_str(), refspec.as_str()]);
        let out = plain(
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(&args)
                // the transports a reference may come by, decided here rather than by a person's git config
                .env("GIT_ALLOW_PROTOCOL", "https:ssh:git:file"),
        )?;
        if !out.status.success() {
            return Ok(RefRead::NotMade(failure(&args, &out)));
        }
        let fetched = verify(&format!("{private}^{{commit}}"))?.unwrap_or_default();
        let _ = git(&dir, &["update-ref", "-d", &private]);
        if fetched != r.commit {
            return Ok(RefRead::NotMade(format!(
                "{} {} is {} now, not the recorded {}",
                r.url, r.reference, fetched, r.commit
            )));
        }
        let zero = "0".repeat(r.commit.len());
        let make = ["update-ref", tag.as_str(), r.commit.as_str(), zero.as_str()];
        let out = git(&dir, &make)?;
        if !out.status.success() {
            return Ok(RefRead::NotMade(failure(&make, &out)));
        }
        Ok(RefRead::Made)
    }

    fn marked(
        &self,
        repo: &RepoName,
        rev: &str,
        attr: &str,
        paths: &[String],
    ) -> Result<BTreeSet<String>, RepoError> {
        if paths.is_empty() {
            return Ok(BTreeSet::new());
        }
        // the attributes at `rev`, as the repository's own .gitattributes there say: paths in on stdin, NUL-ended,
        // and out as PATH, NUL, ATTR, NUL, VALUE, NUL
        let source = format!("--source={rev}");
        let args = ["check-attr", "-z", "--stdin", source.as_str(), attr];
        let mut ask = Command::new("git");
        // the repository's own attributes only: a person's global attributes file marks nothing here
        ask.arg("-C").arg(self.clone_dir(repo)?).args(["-c", "core.attributesFile=/dev/null"]);
        ask.args(args).stdin(Stdio::piped());
        ask.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
        ask.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child =
            ask.spawn().map_err(|e| RepoError::Failed(format!("cannot run git: {e}")))?;
        let input: Vec<u8> = paths.iter().flat_map(|p| p.bytes().chain([0])).collect();
        // written beside the read, never before it: git answers as it reads, and a full pipe each way would wait
        let writer = child.stdin.take().map(|mut stdin| {
            std::thread::spawn(move || {
                use std::io::Write;
                stdin.write_all(&input)
            })
        });
        let out = child
            .wait_with_output()
            .map_err(|e| RepoError::Failed(format!("{repo}: git check-attr: {e}")))?;
        let written = writer.map(|w| w.join());
        // git's own words first: a git that stopped early breaks the pipe, and that says nothing of why
        if !out.status.success() {
            return Err(RepoError::Failed(format!("{repo}: {}", failure(&args, &out))));
        }
        if let Some(Ok(Err(e))) = written {
            return Err(RepoError::Failed(format!("{repo}: git check-attr: {e}")));
        }
        Ok(marked_paths(&String::from_utf8_lossy(&out.stdout)))
    }
}

/// The paths `git check-attr -z` printed as marked: each answer PATH, NUL, ATTR, NUL, VALUE, NUL, marked when its
/// value is `set` or any other value but `unset`, `unspecified` or `false`.
fn marked_paths(out: &str) -> BTreeSet<String> {
    let fields: Vec<&str> = out.split('\0').collect();
    fields
        .chunks_exact(3)
        .filter(|f| !matches!(f[2], "unset" | "unspecified" | "false"))
        .map(|f| f[0].to_string())
        .collect()
}

impl GitRepos<'_> {
    /// git in the clone of `repo`, which must succeed; its stdout.
    fn read(&self, repo: &RepoName, args: &[&str]) -> Result<String, RepoError> {
        let out = git(&self.clone_dir(repo)?, args)?;
        if !out.status.success() {
            return Err(RepoError::Failed(format!("{repo}: {}", failure(args, &out))));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl IntegrationReads for GitRepos<'_> {
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
            _ => Err(RepoError::Failed(format!(
                "{repo}: {}",
                failure(&["merge-base", "--is-ancestor", a, b], &out)
            ))),
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

    fn tag_messages(&self, repo: &RepoName) -> Result<Vec<(String, String)>, RepoError> {
        // one read for every tag: an annotated one's message, each record ended by NUL and git's newline
        let format = "--format=%(objecttype)%00%(refname:strip=2)%00%(contents)%00";
        let out = self.read(repo, &["for-each-ref", format, "refs/tags/"])?;
        Ok(out
            .split("\0\n")
            .filter_map(|record| {
                let mut f = record.splitn(3, '\0');
                match (f.next(), f.next(), f.next()) {
                    (Some("tag"), Some(name), Some(message)) => {
                        Some((name.to_string(), message.to_string()))
                    }
                    _ => None,
                }
            })
            .collect())
    }

    fn ls_remote(
        &self,
        repo: &RepoName,
        remote: &str,
        namespace: &str,
    ) -> Result<Result<Vec<(String, String)>, String>, RepoError> {
        if remote.starts_with('-') {
            return Ok(Err(format!("{remote} starts with a dash")));
        }
        // one ask: a tag's peeled line (NAME^{}) overwrites its own, so each name carries its commit
        let mut ask = Command::new("git");
        ask.arg("-C").arg(self.clone_dir(repo)?).args([
            "ls-remote",
            remote,
            &format!("{namespace}*"),
        ]);
        // a remote that cannot answer is not known yet: never a prompt, never a wait without end
        ask.env("GIT_ASKPASS", "true");
        if std::env::var_os("GIT_SSH_COMMAND").is_none() {
            ask.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=10");
        }
        let out = plain(&mut ask)?;
        if !out.status.success() {
            let why = stderr(&out);
            return Ok(Err(if why.is_empty() {
                "git ls-remote failed, saying nothing".to_string()
            } else {
                why
            }));
        }
        let mut refs: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let Some((sha, name)) = line.split_once('\t') else { continue };
            let Some(name) = name.strip_prefix(namespace) else { continue };
            match name.strip_suffix("^{}") {
                Some(tag) => {
                    refs.insert(tag.to_string(), sha.to_string());
                }
                None => {
                    refs.entry(name.to_string()).or_insert_with(|| sha.to_string());
                }
            }
        }
        Ok(Ok(refs.into_iter().collect()))
    }

    fn is_shallow(&self, repo: &RepoName) -> Result<bool, RepoError> {
        Ok(self.read(repo, &["rev-parse", "--is-shallow-repository"])?.trim() == "true")
    }

    fn has_commit(&self, repo: &RepoName, sha: &str) -> Result<bool, RepoError> {
        let out = git(&self.clone_dir(repo)?, &["cat-file", "-e", &format!("{sha}^{{commit}}")])?;
        Ok(out.status.success())
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
    use std::os::unix::process::ExitStatusExt;

    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn a_failure_is_never_empty() {
        let args = ["fetch", "origin"];
        assert_eq!(failure(&args, &output(128, "", "fatal: no\n")), "git fetch origin: fatal: no");
        assert_eq!(
            failure(&args, &output(1, "", "")),
            "git fetch origin: ended with exit 1, saying nothing"
        );
        let refused = "! a b refs/tags/v1\n  c d refs/tags/v2\n";
        assert_eq!(
            failure(&args, &output(1, refused, "")),
            "git fetch origin: ended with exit 1, saying on stdout: ! a b refs/tags/v1;   c d refs/tags/v2"
        );
    }

    #[test]
    fn a_tag_fetch_is_accepted_only_when_it_refused_tags_and_nothing_else() {
        let tag = "! a1 b1 refs/tags/v1\n  a2 b2 refs/tags/v2\n";
        let moved = vec![("v1".to_string(), "a1".to_string(), "b1".to_string())];
        assert_eq!(fetched_tags(&output(0, "  a2 b2 refs/tags/v2\n", "")), Some(vec![]));
        assert_eq!(fetched_tags(&output(1, tag, "")), Some(moved));
        let branch_too = format!("{tag}! a3 b3 refs/heads/main\n");
        assert_eq!(fetched_tags(&output(1, &branch_too, "")), None, "a refused branch");
        assert_eq!(fetched_tags(&output(1, tag, "error: cannot lock ref\n")), None, "git said why");
        assert_eq!(fetched_tags(&output(128, tag, "")), None, "another end");
        assert_eq!(fetched_tags(&output(1, "", "")), None, "refusing nothing");
        let killed = Output { status: std::process::ExitStatus::from_raw(9), ..output(1, tag, "") };
        assert_eq!(fetched_tags(&killed), None, "a signal");
    }

    #[test]
    fn a_path_is_marked_when_its_attribute_is_set_or_given_a_value() {
        let out = "a\0r\0set\0b\0r\0unspecified\0c\0r\0unset\0d\0r\0yes\0e\0r\0false\0";
        let want: BTreeSet<String> = ["a", "d"].map(String::from).into();
        assert_eq!(marked_paths(out), want);
        assert!(marked_paths("").is_empty());
    }

    #[test]
    fn grep_hits_are_read_field_by_field() {
        let out = "origin/main:a.md\x002\x00uses v1: here\norigin/main:odd\nname:x\x0010\x00v1\n";
        assert_eq!(
            grep_hits(out, "origin/main"),
            vec![
                ("a.md".to_string(), 2, "uses v1: here".to_string()),
                ("odd\nname:x".to_string(), 10, "v1".to_string()),
            ]
        );
        assert_eq!(grep_hits("", "origin/main"), vec![]);
    }

    #[test]
    fn only_refused_tags_are_moved_tags() {
        let porcelain = "! a1 b1 refs/tags/v1\n  a2 b2 refs/tags/v2\n! a3 b3 refs/heads/main\n* 0 c4 refs/tags/v3\n";
        assert_eq!(
            moved_tags(porcelain),
            vec![("v1".to_string(), "a1".to_string(), "b1".to_string())]
        );
    }

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
