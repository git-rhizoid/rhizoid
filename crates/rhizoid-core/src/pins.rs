//! Pins: where a follower writes the release of a tool it uses, read and moved in one place - the file and the
//! key or line the graph names, and only the pinned value changed, proven by reading it back.

use crate::graph::{PinRead, RepoName, Tool};
use crate::repos::RepoError;

/// The version a pin file names for `tool`: its key's value (the `value` template stripped) or its line's;
/// `None` when the file names none.
pub fn read_pin(tool: &Tool, follower: &RepoName, text: &str) -> Result<Option<String>, RepoError> {
    match &tool.read {
        PinRead::Key(path) => {
            let table = text.parse::<toml::Table>().map_err(|e| {
                RepoError::Failed(format!("{follower}: {} is not valid TOML: {e}", tool.pin))
            })?;
            let mut v = table.get(&path[0]);
            for k in &path[1..] {
                v = v.and_then(|x| x.get(k));
            }
            match v {
                None => Ok(None),
                Some(toml::Value::String(s)) => {
                    let (before, after) = &tool.value;
                    s.strip_prefix(before.as_str())
                        .and_then(|rest| rest.strip_suffix(after.as_str()))
                        .map(|version| Some(version.to_string()))
                        .ok_or_else(|| {
                            RepoError::Failed(format!(
                                "{follower}: {}: `{}` is {s:?}, not {before}VERSION{after}",
                                tool.pin,
                                path.join(".")
                            ))
                        })
                }
                Some(_) => Err(RepoError::Failed(format!(
                    "{follower}: {}: `{}` is not a string",
                    tool.pin,
                    path.join(".")
                ))),
            }
        }
        PinRead::Line { before, after } => Ok(text.lines().find_map(|l| {
            let inner = l.strip_prefix(before.as_str())?.strip_suffix(after.as_str())?;
            Some(inner.to_string())
        })),
    }
}

/// The pin file's text with `tool`'s version moved from `old` to `new`: only the pinned value changes, the rest
/// of the file as it was, and the edit is read back before it is given.
pub fn edit_pin(tool: &Tool, text: &str, old: &str, new: &str) -> Result<String, String> {
    let follower = RepoName::try_from("pin/edit".to_string()).expect("a placeholder name");
    let (from, to) = match &tool.read {
        PinRead::Key(_) => {
            let (before, after) = &tool.value;
            (format!("{before}{old}{after}"), format!("{before}{new}{after}"))
        }
        PinRead::Line { before, after } => {
            (format!("{before}{old}{after}"), format!("{before}{new}{after}"))
        }
    };
    let edited = match &tool.read {
        PinRead::Key(_) => {
            // the value as TOML writes it, in either kind of quotes; it must appear once
            let quoted = [format!("\"{from}\""), format!("'{from}'")];
            let hits: Vec<usize> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| quoted.iter().any(|q| code_part(l).contains(q.as_str())))
                .map(|(i, _)| i)
                .collect();
            let [at] = hits.as_slice() else {
                return Err(format!(
                    "{} holds {from:?} {} times, not once: move it by hand",
                    tool.pin,
                    hits.len()
                ));
            };
            replace_line(text, *at, |l| {
                l.replacen(&quoted[0], &format!("\"{to}\""), 1).replacen(
                    &quoted[1],
                    &format!("'{to}'"),
                    1,
                )
            })
        }
        PinRead::Line { .. } => {
            let hits: Vec<usize> =
                text.lines().enumerate().filter(|(_, l)| *l == from).map(|(i, _)| i).collect();
            let [at] = hits.as_slice() else {
                return Err(format!(
                    "{} has the line {from:?} {} times, not once: move it by hand",
                    tool.pin,
                    hits.len()
                ));
            };
            replace_line(text, *at, |_| to.clone())
        }
    };
    let read_back = matches!(read_pin(tool, &follower, &edited), Ok(Some(v)) if v == new);
    if !read_back || !only_the_key_moved(tool, text, &edited, &to) {
        return Err(format!(
            "{}: the edit did not read back as {new} alone: move it by hand",
            tool.pin
        ));
    }
    Ok(edited)
}

/// For a key pin, the edited file parses as the original with that one key set to `to`: nothing else moved.
fn only_the_key_moved(tool: &Tool, original: &str, edited: &str, to: &str) -> bool {
    let PinRead::Key(path) = &tool.read else { return true };
    let (Ok(mut want), Ok(got)) = (original.parse::<toml::Table>(), edited.parse::<toml::Table>())
    else {
        return false;
    };
    let mut at: &mut toml::Table = &mut want;
    for k in &path[..path.len() - 1] {
        match at.get_mut(k) {
            Some(toml::Value::Table(t)) => at = t,
            _ => return false,
        }
    }
    at.insert(path[path.len() - 1].clone(), toml::Value::String(to.to_string()));
    want == got
}

/// A TOML line without its comment: cut at the first `#` outside quotes.
fn code_part(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '#') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn replace_line(text: &str, at: usize, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if i == at {
            let (body, end) = match line.strip_suffix("\r\n") {
                Some(b) => (b, "\r\n"),
                None => match line.strip_suffix('\n') {
                    Some(b) => (b, "\n"),
                    None => (line, ""),
                },
            };
            out.push_str(&f(body));
            out.push_str(end);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// What kind of pin: a tool's version, which `bump` moves, or a slice's release, which `follow` moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A tool pinned at a release of its source.
    Tool,
    /// A slice of files taken at a release of its source.
    Slice,
}

/// One pin of the graph: who pins what, where, at which release, and the newest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pin {
    /// The repository that pins.
    pub follower: RepoName,
    /// What it is.
    pub kind: Kind,
    /// The tool's name, or the slice's source.
    pub name: String,
    /// The file that holds the pin.
    pub file: String,
    /// The release it names (`None`: the file names none).
    pub pinned: Option<String>,
    /// The newest release of its source.
    pub latest: String,
    /// The tool, for a tool pin: where to find it in the graph.
    #[serde(skip)]
    pub tool: Option<usize>,
}

impl Pin {
    /// Whether it names the newest release.
    pub fn current(&self) -> bool {
        self.pinned.as_deref() == Some(self.latest.as_str())
    }
}

/// Every pin of the graph, read at the graph's revision of each follower's clone: its tools' and its slices'.
pub fn plan(graph: &crate::Graph, repos: &dyn crate::Repos) -> Result<Vec<Pin>, RepoError> {
    let rev: &str = &graph.rev;
    let latest = |source: &RepoName, prefix: &str| -> Result<String, RepoError> {
        crate::text::latest_release(&repos.tags(source)?, prefix).ok_or_else(|| {
            RepoError::Failed(format!("{source} has no release tagged {prefix}N.N.N"))
        })
    };
    let mut pins = Vec::new();
    for (i, t) in graph.tools.iter().enumerate() {
        let newest = latest(&t.source, &t.tag_prefix)?;
        for f in &t.followers {
            let text = repos.file(f, rev, &t.pin)?.unwrap_or_default();
            pins.push(Pin {
                follower: f.clone(),
                kind: Kind::Tool,
                name: t.name.clone(),
                file: t.pin.clone(),
                pinned: read_pin(t, f, &text)?,
                latest: newest.clone(),
                tool: Some(i),
            });
        }
    }
    for s in &graph.slices {
        let newest = latest(&s.source, &s.tag_prefix)?;
        for f in &s.followers {
            let text = repos.file(f, rev, &s.pin)?.unwrap_or_default();
            let table = text
                .parse::<toml::Table>()
                .map_err(|e| RepoError::Failed(format!("{f}: {} is not valid TOML: {e}", s.pin)))?;
            pins.push(Pin {
                follower: f.clone(),
                kind: Kind::Slice,
                name: s.source.to_string(),
                file: s.pin.clone(),
                pinned: table.get("version").and_then(|v| v.as_str()).map(str::to_string),
                latest: newest.clone(),
                tool: None,
            });
        }
    }
    Ok(pins)
}

/// How a tool pin's bump ended.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Bumped {
    /// Committed on its branch (and pushed, and its pull request opened, when asked).
    Bumped {
        /// The branch.
        branch: String,
        /// Pushed to `origin`.
        pushed: bool,
        /// What the pull request command printed (its address).
        pr: Option<String>,
        /// The pull request command failed, the branch pushed: open it by hand.
        pr_error: Option<String>,
    },
    /// Its branch exists already: an earlier bump, not merged yet.
    BranchExists {
        /// The branch.
        branch: String,
    },
    /// The gate failed: nothing was committed.
    GatedOut {
        /// The gate's exit code.
        exit: Option<i32>,
        /// What it printed.
        output: String,
    },
    /// The edit was not certain (the value twice, or not read back): moved by hand.
    Refused {
        /// Why.
        why: String,
    },
}

/// Bump one tool pin to its newest release in the follower's clone: a branch from the graph's revision, in a
/// fresh worktree; only the pinned value edited and read back; the gate, when the tool has one, passed before
/// the commit; pushed and its pull request opened only when asked. The worktree always goes.
pub fn bump(
    git: &impl crate::GitPort,
    cmd: &impl crate::CommandPort,
    tool: &Tool,
    pin: &Pin,
    at: &BumpAt,
) -> Result<Bumped, crate::PortError> {
    let old = pin.pinned.clone().unwrap_or_default();
    let branch = format!("rhizoid/bump/{}-{}", tool.name, pin.latest);
    let here = git.commit(&format!("refs/heads/{branch}"))?.is_some();
    if here || git.commit(&format!("refs/remotes/origin/{branch}"))?.is_some() {
        return Ok(Bumped::BranchExists { branch });
    }
    let base = git.commit(at.rev)?.ok_or_else(|| {
        crate::PortError(format!("{}: no {} in the clone (fetch it)", pin.follower, at.rev))
    })?;
    let worktree = at.home.join(branch.replace('/', "-"));
    git.add_branch_worktree(&worktree, &branch, &base)?;
    let outcome = edit_gate_commit(git, cmd, tool, pin, at, &worktree, &branch, &old);
    let _ = git.remove_worktree(&worktree);
    match outcome {
        // once committed, the branch is kept: pushed or not, it is the bump
        Ok(b @ Bumped::Bumped { .. }) => Ok(b),
        Ok(other) => {
            git.delete_branch(&branch)?;
            Ok(other)
        }
        Err(e) => {
            let _ = git.delete_branch(&branch);
            Err(e)
        }
    }
}

/// Where a bump happens and how far it goes.
pub struct BumpAt<'a> {
    /// The revision the branch starts from (`origin/main`).
    pub rev: &'a str,
    /// rhizoid's worktree home for the follower's clone.
    pub home: &'a std::path::Path,
    /// Push the branch to `origin`.
    pub push: bool,
    /// The pull request command (argv, `{branch}` in it replaced), run once pushed.
    pub pr: Option<&'a [String]>,
}

#[allow(clippy::too_many_arguments)]
fn edit_gate_commit(
    git: &impl crate::GitPort,
    cmd: &impl crate::CommandPort,
    tool: &Tool,
    pin: &Pin,
    at: &BumpAt,
    worktree: &std::path::Path,
    branch: &str,
    old: &str,
) -> Result<Bumped, crate::PortError> {
    let path = worktree.join(&pin.file);
    let io = |e: std::io::Error| crate::PortError(format!("{}: {e}", path.display()));
    let text = std::fs::read_to_string(&path).map_err(io)?;
    let edited = match edit_pin(tool, &text, old, &pin.latest) {
        Ok(t) => t,
        Err(why) => return Ok(Bumped::Refused { why }),
    };
    std::fs::write(&path, edited).map_err(io)?;
    if let Some(gate) = &tool.gate {
        let (exit, stdout, stderr) = cmd.run(gate, worktree)?;
        if exit != Some(0) {
            return Ok(Bumped::GatedOut { exit, output: format!("{stdout}{stderr}") });
        }
    }
    // the commit is the pin alone: a gate that changed anything else is refused, not swept in
    let others: Vec<String> =
        git.changed_files(worktree)?.into_iter().filter(|f| *f != pin.file).collect();
    if !others.is_empty() {
        return Ok(Bumped::Refused {
            why: format!("the bump changed more than the pin: {}", others.join(", ")),
        });
    }
    git.commit_all(worktree, &format!("chore(deps): {} {old} -> {}", tool.name, pin.latest))?;
    let pushed = at.push || at.pr.is_some();
    let mut bumped =
        Bumped::Bumped { branch: branch.to_string(), pushed: false, pr: None, pr_error: None };
    if pushed {
        git.push(worktree, branch)?;
        bumped =
            Bumped::Bumped { branch: branch.to_string(), pushed: true, pr: None, pr_error: None };
    }
    if let Some(command) = at.pr {
        let argv: Vec<String> = command.iter().map(|a| a.replace("{branch}", branch)).collect();
        let (pr, pr_error) = match cmd.run(&argv, worktree) {
            Ok((Some(0), stdout, _)) => (Some(stdout.trim().to_string()), None),
            Ok((_, stdout, stderr)) => (None, Some(format!("{stdout}{stderr}").trim().to_string())),
            Err(e) => (None, Some(e.0)),
        };
        bumped = Bumped::Bumped { branch: branch.to_string(), pushed: true, pr, pr_error };
    }
    Ok(bumped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Graph;

    fn tool(edge: &str) -> Tool {
        Graph::parse(&format!("[[tool]]\nname = \"t\"\nsource = \"a/b\"\ntag_prefix = \"t-v\"\nfollowers = [\"c/d\"]\n{edge}"))
            .unwrap()
            .tools
            .remove(0)
    }

    fn name() -> RepoName {
        RepoName::try_from("c/d".to_string()).unwrap()
    }

    #[test]
    fn a_key_pin_moves_and_nothing_else_does() {
        let t = tool("pin = \"lab.toml\"\nkey = \"tools.lpoco\"\n");
        let text = "# keep me\n[tools]\nlpoco = \"0.42.2\" # pinned\nother = \"0.42.2-x\"\n";
        assert_eq!(read_pin(&t, &name(), text).unwrap().as_deref(), Some("0.42.2"));
        let moved = edit_pin(&t, text, "0.42.2", "0.43.0").unwrap();
        assert_eq!(
            moved,
            "# keep me\n[tools]\nlpoco = \"0.43.0\" # pinned\nother = \"0.42.2-x\"\n"
        );
    }

    #[test]
    fn a_cargo_git_tag_pin_keeps_its_prefix() {
        let t = tool("pin = \"Cargo.toml\"\nkey = \"dependencies.lpoco-attended.tag\"\nvalue = \"lpoco-attended-v{version}\"\n");
        let text = "[dependencies]\nlpoco-attended = { git = \"https://x/lab-tools\", tag = \"lpoco-attended-v0.34.1\" }\n";
        assert_eq!(read_pin(&t, &name(), text).unwrap().as_deref(), Some("0.34.1"));
        let moved = edit_pin(&t, text, "0.34.1", "0.41.0").unwrap();
        assert!(moved.contains("tag = \"lpoco-attended-v0.41.0\""), "{moved}");
        assert!(moved.contains("git = \"https://x/lab-tools\""));
    }

    #[test]
    fn a_line_pin_moves_its_line() {
        let t = tool("pin = \"bin/install.sh\"\nline = 'version=\"${V:-{version}}\"'\n");
        let text = "#!/bin/sh\nversion=\"${V:-0.7.1}\"\necho $version\n";
        assert_eq!(
            edit_pin(&t, text, "0.7.1", "0.8.0").unwrap(),
            "#!/bin/sh\nversion=\"${V:-0.8.0}\"\necho $version\n"
        );
    }

    #[test]
    fn a_comment_or_crlf_does_not_get_in_the_way() {
        let t = tool("pin = \"lab.toml\"\nkey = \"tools.lpoco\"\n");
        let text = "[tools]\n# was \"0.42.2\" before\r\nlpoco = \"0.42.2\"\r\n";
        assert_eq!(
            edit_pin(&t, text, "0.42.2", "0.43.0").unwrap(),
            "[tools]\n# was \"0.42.2\" before\r\nlpoco = \"0.43.0\"\r\n"
        );
        let line = tool("pin = \"i.sh\"\nline = \"v={version}\"\n");
        assert_eq!(edit_pin(&line, "a\r\nv=1\r\nb\r\n", "1", "2").unwrap(), "a\r\nv=2\r\nb\r\n");
    }

    #[test]
    fn an_edit_that_moves_another_key_too_is_refused() {
        let t = tool("pin = \"C.toml\"\nkey = \"d.x.tag\"\nvalue = \"x-v{version}\"\n");
        let text = "[d]\nx = { tag = \"x-v1\", rev = \"x-v1\" }\n";
        let moved = edit_pin(&t, text, "1", "2").unwrap();
        assert!(
            moved.contains("tag = \"x-v2\"") && moved.contains("rev = \"x-v1\""),
            "only the key: {moved}"
        );
    }

    #[test]
    fn an_edit_that_is_not_certain_is_refused() {
        let t = tool("pin = \"lab.toml\"\nkey = \"tools.lpoco\"\n");
        let twice = "[tools]\nlpoco = \"0.42.2\"\n[other]\nx = \"0.42.2\"\n";
        assert!(edit_pin(&t, twice, "0.42.2", "0.43.0").unwrap_err().contains("2 times"));
        let prefixed = tool("pin = \"C.toml\"\nkey = \"d.x.tag\"\nvalue = \"x-v{version}\"\n");
        assert!(
            read_pin(&prefixed, &name(), "[d]\nx = { tag = \"y-v1\" }\n").is_err(),
            "not the template's shape"
        );
        assert!(Graph::parse("[[tool]]\nname = \"t\"\nsource = \"a/b\"\ntag_prefix = \"v\"\npin = \"p\"\nkey = \"k\"\nvalue = \"no-version\"\nfollowers = []\n").is_err());
    }
}
