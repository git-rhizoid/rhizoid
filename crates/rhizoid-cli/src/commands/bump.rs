//! `rhizoid bump [now]` - every pin of a graph in one place: each tool's and each slice's, what it names and the
//! newest release. With `now`, each tool pin that is behind is bumped on a branch of its follower's clone (gated
//! when the tool says so), pushed and its pull request opened when asked. Slices are moved by `follow`.
//! It ends 0 when every pin is current (or, with `now`, every tool pin bumped), 1 when not, 2 when it could not run.

use crate::git_cli::{GitCli, Programs};
use crate::git_repos::GitRepos;
use rhizoid_core::pins::{bump, plan, BumpAt, Bumped, Kind, Pin};
use rhizoid_core::Graph;
use std::path::PathBuf;

/// What bump was asked, with every default already decided.
pub struct Ask {
    pub graph: PathBuf,
    pub root: PathBuf,
    pub worktrees: PathBuf,
    pub now: bool,
    pub push: bool,
    pub pr: bool,
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
    if !ask.now && (ask.push || ask.pr) {
        return Err(
            "--push and --pr change followers' remotes: they go with `bump now`".to_string()
        );
    }
    let text = std::fs::read_to_string(&ask.graph)
        .map_err(|e| format!("cannot read the graph file {}: {e}", ask.graph.display()))?;
    let graph = Graph::parse(&text).map_err(|e| format!("{}: {e}", ask.graph.display()))?;
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
        if ask.now && row.pin.kind == Kind::Tool && !row.pin.current() {
            // one pin's failure is its own: the others are still bumped, and every one is reported
            match bump_one(ask, &graph, &row.pin) {
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
    crate::commands::status::write_report(&mut std::io::stdout().lock(), &render(&rows, ask.json))?;
    Ok(rows.iter().all(|r| match r.state {
        "current" | "branch-exists" => true,
        "bumped" => !matches!(&r.bump, Some(Bumped::Bumped { pr_error: Some(_), .. })),
        "behind" => ask.now && r.pin.kind == Kind::Slice,
        _ => false,
    }))
}

fn bump_one(ask: &Ask, graph: &Graph, pin: &Pin) -> Result<Bumped, String> {
    let tool = &graph.tools[pin.tool.expect("a tool pin names its tool")];
    let clone = ask.root.join(graph.clone_path(&pin.follower));
    let git = GitCli::new(&clone);
    let main = git.main_clone().map_err(|e| e.to_string())?;
    let home = crate::commands::assemble::rhizoid_home(&ask.worktrees, &main)?;
    let pr: Vec<String> = PR.iter().map(|s| s.to_string()).collect();
    let at = BumpAt {
        rev: &graph.rev,
        home: &home,
        push: ask.push,
        pr: ask.pr.then_some(pr.as_slice()),
    };
    bump(&git, &Programs, tool, pin, &at).map_err(|e| e.to_string())
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
        if r.pin.kind == Kind::Slice && r.state == "behind" {
            put(k("moved_by"), "follow");
        }
        match &r.bump {
            Some(Bumped::Bumped { branch, pushed, pr, pr_error }) => {
                put(k("branch"), branch);
                put(k("pushed"), if *pushed { "yes" } else { "no" });
                if let Some(pr) = pr {
                    put(k("pr"), pr);
                }
                if let Some(e) = pr_error {
                    put(k("pr_error"), e.lines().last().unwrap_or(""));
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
