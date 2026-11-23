//! `rhizoid bump [now]` - every pin of a graph in one place: each tool's and each slice's, what it names and the
//! newest release. With `now`, each pin that is behind and names `rhizoid-bump` as its mover is bumped on a branch of its follower's clone (gated
//! when the tool says so), pushed and its pull request opened when asked. Slices are moved by `follow`.
//! It ends 0 when every pin is current (or, with `now`, every pin it moves bumped, or refused only for a standing
//! reason status reports as drift: its tag name copied), 1 when this run failed at something, 2 when it could not run.

use crate::git_cli::{GitCli, Programs};
use crate::git_repos::GitRepos;
use rhizoid_core::pins::{bump, bump_slice, plan, BumpAt, Bumped, Kind, Pin};
use rhizoid_core::{Graph, Repos};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What bump was asked, with every default already decided.
pub struct Ask {
    pub graph: PathBuf,
    pub root: PathBuf,
    pub worktrees: PathBuf,
    pub now: bool,
    pub push: bool,
    pub pr: bool,
    pub land: bool,
    pub only: Option<String>,
    pub fetch: bool,
    pub json: bool,
}

/// The pull request command: gh, filled from the bump's commit.
const PR: &[&str] = &["gh", "pr", "create", "--fill", "--head", "{branch}"];

/// The draft pull request command for a bump whose gate failed: gh, its title and body rhizoid's.
const DRAFT: &[&str] = &[
    "gh", "pr", "create", "--draft", "--head", "{branch}", "--title", "{title}", "--body", "{body}",
];

/// One pin and, with `now`, how its bump ended.
#[derive(serde::Serialize)]
struct Row {
    #[serde(flatten)]
    pin: Pin,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    bump: Option<Bumped>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// The earlier pull requests of the same pin this bump's replaced, closed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    replaced: Vec<String>,
    /// Earlier pull requests could not be read or closed: why.
    #[serde(skip_serializing_if = "Option::is_none")]
    replace_error: Option<String>,
}

/// Print every pin; whether all is as asked.
pub fn run(ask: &Ask) -> Result<bool, String> {
    if !ask.now && (ask.push || ask.pr || ask.land) {
        return Err("--push, --pr and --land change followers' remotes: they go with `bump now`"
            .to_string());
    }
    let text = std::fs::read_to_string(&ask.graph)
        .map_err(|e| format!("cannot read the graph file {}: {e}", ask.graph.display()))?;
    let graph = Graph::parse(&text).map_err(|e| format!("{}: {e}", ask.graph.display()))?;
    if ask.land && graph.bump.land.is_none() {
        return Err(format!(
            "--land runs the graph's [bump] land command, and {} names none (land = [\"land\"])",
            ask.graph.display()
        ));
    }
    let repos = GitRepos::new(&ask.root, &graph, ask.fetch);
    let pins: Vec<Pin> = plan(&graph, &repos)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|p| ask.only.as_deref().map_or(true, |o| p.follower.as_str() == o))
        .collect();
    if let (Some(only), true) = (&ask.only, pins.is_empty()) {
        return Err(format!("--only {only}: no pin of the graph is that follower's"));
    }
    let mut rows = Vec::new();
    for pin in pins {
        let mut row = Row {
            state: if pin.current() { "current" } else { "behind" },
            bump: None,
            error: None,
            replaced: Vec::new(),
            replace_error: None,
            pin,
        };
        // a pin another job moves is listed, never moved here: each pin has one mover
        if ask.now && !row.pin.current() && row.pin.moved_by_bump() {
            // one pin's failure is its own: the others are still bumped, and every one is reported
            match bump_unless_copied(ask, &graph, &row.pin) {
                Ok(outcome) => {
                    row.state = match &outcome {
                        Bumped::Bumped { .. } => "bumped",
                        Bumped::BranchExists { .. } => "branch-exists",
                        Bumped::GatedOut { .. } => "gated-out",
                        Bumped::Refused { .. } => "refused",
                    };
                    // a newer bump's pull request replaces the earlier ones of the same pin
                    if let Some((branch, url)) = opened(&outcome) {
                        let clone = ask.root.join(graph.clone_path(&row.pin.follower));
                        (row.replaced, row.replace_error) =
                            replace_earlier(&clone, branch, &row.pin.latest, url);
                    }
                    row.bump = Some(outcome);
                }
                Err(e) => {
                    row.state = "error";
                    row.error = Some(e);
                }
            }
        }
        rows.push(row);
    }
    let mut report = render(&rows, ask.json);
    // a tag origin moved that a clone holds as it was is kept and said: after the pins in the text report, on
    // stderr beside the JSON one, whose stdout stays the pins; the exit still says only how the pins stand
    for (i, finding) in repos.fetch_findings().iter().enumerate() {
        if ask.json {
            eprintln!("rhizoid: {finding}");
        } else {
            report.push_str(&format!("finding.{}={finding}\n", i + 1));
        }
    }
    crate::commands::status::write_report(&mut std::io::stdout().lock(), &report)?;
    // with `now`, how the pins another job moves stand is that job's to say
    // an earlier pull request left open is a failure of this run: the next run says branch-exists and never retries
    Ok(rows.iter().filter(|r| !ask.now || r.pin.moved_by_bump()).all(|r| {
        r.replace_error.is_none()
            && match r.state {
                "current" | "branch-exists" => true,
                "bumped" => match &r.bump {
                    Some(Bumped::Bumped { pr_error, landed, .. }) => {
                        pr_error.is_none() && (*landed || !ask.land)
                    }
                    _ => false,
                },
                // refused for a standing reason status reports as drift (pin-copies): status and patrol carry it, and a
                // run that failed at nothing is no failure - an hourly unit stays green for a known finding
                "refused" => ask.now && !r.pin.copies.is_empty(),
                _ => false,
            }
    }))
}

/// A pin whose tag name is written in more places than its pin file is refused: bump would move one copy of several.
fn bump_unless_copied(ask: &Ask, graph: &Graph, pin: &Pin) -> Result<Bumped, String> {
    if pin.copies.is_empty() {
        return bump_one(ask, graph, pin);
    }
    Ok(Bumped::Refused {
        why: format!(
            "its tag name is also written in {}: move every copy to one place, then bump",
            pin.copies.join(", ")
        ),
    })
}

fn bump_one(ask: &Ask, graph: &Graph, pin: &Pin) -> Result<Bumped, String> {
    let clone = ask.root.join(graph.clone_path(&pin.follower));
    let git = GitCli::new(&clone);
    let main = git.main_clone().map_err(|e| e.to_string())?;
    let home = crate::commands::assemble::rhizoid_home(&ask.worktrees, &main)?;
    let pr: Vec<String> = PR.iter().map(|s| s.to_string()).collect();
    let draft: Vec<String> = DRAFT.iter().map(|s| s.to_string()).collect();
    let at = BumpAt {
        rev: &graph.rev,
        home: &home,
        push: ask.push,
        pr: (ask.pr || ask.land).then_some(pr.as_slice()),
        land: if ask.land { graph.bump.land.as_deref() } else { None },
        draft: (ask.pr || ask.land).then_some(draft.as_slice()),
    };
    match (pin.tool, pin.slice) {
        (Some(t), _) => bump(&git, &Programs, &graph.tools[t], pin, &at).map_err(|e| e.to_string()),
        (None, Some(s)) => {
            let slice = &graph.slices[s];
            let source = ask.root.join(graph.clone_path(&slice.source));
            let source =
                std::fs::canonicalize(&source).map_err(|e| format!("{}: {e}", source.display()))?;
            bump_slice(&git, &GitCli::new(&source), &Programs, slice, &source, pin, &at)
                .map_err(|e| e.to_string())
        }
        (None, None) => Err("a pin names neither a tool nor a slice".to_string()),
    }
}

fn render(rows: &[Row], json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(rows).expect("rows serialise") + "\n";
    }
    let mut out = String::new();
    for (i, r) in rows.iter().enumerate() {
        let k = |f: &str| format!("pin.{}.{f}", i + 1);
        let mut put = |key: String, v: &str| out.push_str(&format!("{key}={v}\n"));
        put(k("follower"), r.pin.follower.as_str());
        put(k("moved_by"), &r.pin.moved_by);
        if !r.pin.copies.is_empty() {
            put(k("copies"), &r.pin.copies.join(" "));
        }
        put(
            k("kind"),
            match r.pin.kind {
                Kind::Tool => "tool",
                Kind::Slice => "slice",
            },
        );
        put(k("name"), &r.pin.name);
        put(k("file"), &r.pin.file);
        put(k("pinned"), r.pin.pinned.as_deref().unwrap_or("none"));
        put(k("latest"), &r.pin.latest);
        put(k("state"), r.state);
        match &r.bump {
            Some(Bumped::Bumped { branch, pushed, pr, pr_error, landed, land_error }) => {
                put(k("branch"), branch);
                put(k("pushed"), if *pushed { "yes" } else { "no" });
                if let Some(pr) = pr {
                    put(k("pr"), pr);
                }
                if let Some(e) = pr_error {
                    put(k("pr_error"), e.lines().last().unwrap_or(""));
                }
                if *landed {
                    put(k("landed"), "yes");
                }
                if let Some(e) = land_error {
                    put(k("landed"), "no");
                    put(k("land_error"), e.lines().last().unwrap_or(""));
                }
            }
            Some(Bumped::BranchExists { branch }) => put(k("branch"), branch),
            Some(Bumped::GatedOut { exit, output, branch, pr, pr_error }) => {
                put(k("gate.exit"), &exit.map_or("signal".to_string(), |e| e.to_string()));
                // all of it, a line each, so the journal holds what the gate said
                if output.is_empty() {
                    put(k("gate.output"), "");
                }
                for line in rhizoid_core::pins::shown_lines(output) {
                    put(k("gate.output"), &line);
                }
                if let Some(branch) = branch {
                    put(k("branch"), branch);
                    put(k("pushed"), "yes");
                }
                if let Some(pr) = pr {
                    put(k("pr"), pr);
                    put(k("draft"), "yes");
                }
                if let Some(e) = pr_error {
                    put(k("pr_error"), e.lines().last().unwrap_or(""));
                }
            }
            Some(Bumped::Refused { why }) => put(k("why"), why),
            None => {}
        }
        if !r.replaced.is_empty() {
            put(k("replaced"), &r.replaced.join(" "));
        }
        if let Some(e) = &r.replace_error {
            put(k("replace_error"), e.lines().last().unwrap_or(""));
        }
        if let Some(e) = &r.error {
            put(k("error"), e);
        }
    }
    out
}

/// The branch and pull request a bump opened, landed or not: a draft for a failed gate too.
fn opened(outcome: &Bumped) -> Option<(&str, &str)> {
    match outcome {
        Bumped::Bumped { branch, pr: Some(pr), .. } => Some((branch, pr)),
        Bumped::GatedOut { branch: Some(branch), pr: Some(pr), .. } => Some((branch, pr)),
        _ => None,
    }
}

/// Close the open pull requests of the same pin at an earlier release - branches `PREFIX` + another version, the
/// prefix being this bump's branch without its release - each with a comment naming the one that replaces it.
fn replace_earlier(
    clone: &Path,
    branch: &str,
    latest: &str,
    url: &str,
) -> (Vec<String>, Option<String>) {
    let mut closed = Vec::new();
    let Some(prefix) = branch.strip_suffix(latest) else { return (closed, None) };
    let gh = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("gh")
            .args(args)
            .current_dir(clone)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run gh: {e}"))?;
        if !out.status.success() {
            let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(format!("gh {}: {said}", args.join(" ")));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let list =
        gh(&["pr", "list", "--state", "open", "--limit", "200", "--json", "number,headRefName"]);
    let open: Vec<serde_json::Value> = match list
        .and_then(|l| serde_json::from_str(&l).map_err(|e| format!("gh pr list said no JSON: {e}")))
    {
        Ok(open) => open,
        Err(e) => return (closed, Some(e)),
    };
    for pr in open {
        let (Some(number), Some(head)) = (pr["number"].as_u64(), pr["headRefName"].as_str()) else {
            continue;
        };
        let earlier = head != branch && head.strip_prefix(prefix).is_some_and(is_release);
        if earlier {
            let comment = format!("Replaced by {url}, the bump to {latest}.");
            if let Err(e) = gh(&["pr", "close", &number.to_string(), "--comment", &comment]) {
                return (closed, Some(e));
            }
            closed.push(format!("#{number}"));
        }
    }
    (closed, None)
}

/// Whether what follows a pin's branch prefix is a release: numbers joined by dots, then the end or a suffix after
/// `-` or `+` (`0.44.5`, `0.43.0-cut.2`) - never another pin's name (`20-1.0.0` of a tool `node-20` beside `node`).
fn is_release(rest: &str) -> bool {
    let head = rest.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = head.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::is_release;

    #[test]
    fn only_a_release_follows_a_pins_prefix() {
        for yes in ["0.44.5", "1.0", "0.43.0-cut.2", "2.0.0+build"] {
            assert!(is_release(yes), "{yes}");
        }
        for no in ["20-1.0.0", "attended-0.2.0", "1", "", "1.", "v1.0"] {
            assert!(!is_release(no), "{no}");
        }
    }
}
