//! `rhizoid bump [now]` - every pin of a graph in one place: each tool's and each slice's, what it names and the
//! newest release. With `now`, each pin that is behind and names `rhizoid-bump` as its mover is bumped on a branch of its follower's clone (gated
//! when the tool says so), pushed and its pull request opened when asked. Slices are moved by `follow`.
//! It ends 0 when every pin is current (or, with `now`, every pin it moves bumped), 1 when not, 2 when it could not run.

use crate::git_cli::{GitCli, Programs};
use crate::git_repos::GitRepos;
use rhizoid_core::pins::{bump, bump_slice, plan, BumpAt, Bumped, Kind, Pin};
use rhizoid_core::{Graph, Repos};
use std::path::PathBuf;

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
    Ok(rows.iter().filter(|r| !ask.now || r.pin.moved_by_bump()).all(|r| match r.state {
        "current" | "branch-exists" => true,
        "bumped" => match &r.bump {
            Some(Bumped::Bumped { pr_error, landed, .. }) => {
                pr_error.is_none() && (*landed || !ask.land)
            }
            _ => false,
        },
        _ => false,
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
    let at = BumpAt {
        rev: &graph.rev,
        home: &home,
        push: ask.push,
        pr: (ask.pr || ask.land).then_some(pr.as_slice()),
        land: if ask.land { graph.bump.land.as_deref() } else { None },
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
            Some(Bumped::GatedOut { exit, output }) => {
                put(k("gate.exit"), &exit.map_or("signal".to_string(), |e| e.to_string()));
                put(k("gate.output"), output.lines().last().unwrap_or(""));
            }
            Some(Bumped::Refused { why }) => put(k("why"), why),
            None => {}
        }
        if let Some(e) = &r.error {
            put(k("error"), e);
        }
    }
    out
}
