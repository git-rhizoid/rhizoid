//! The cutting verbs (rhizcut): `add`, `import`, `update`, `refresh` and `remove`, on the cuttings the graph records
//! in its `[[fork]]` edges. Each prints its plan; `now` carries it out. GitHub only through gh (GhCli).
//! It ends 0 when the plan can go (or went), 1 when it is blocked, 2 when it could not run.

use crate::gh_cli::GhCli;
use crate::git_cli::{GitCli, Programs};
use rhizoid_core::cutting::{
    carry_out, plan_add, plan_import, plan_remove, plan_update, refresh, Ask as CoreAsk,
    ClonePlace, Doing, Plan,
};
use rhizoid_core::{Graph, RepoName};
use std::path::PathBuf;

/// Which verb.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Add,
    Import,
    Update,
    Refresh,
    Remove,
}

/// What a cutting verb was asked, with every default already decided.
pub struct Ask {
    pub verb: Verb,
    pub target: Option<String>,
    pub now: bool,
    pub graph: PathBuf,
    pub root: PathBuf,
    pub worktrees: PathBuf,
    pub org: Option<String>,
    pub tracking: Option<String>,
    pub upstream: Option<String>,
    pub upstream_branch: Option<String>,
    pub patches: Option<String>,
    pub json: bool,
}

/// Print the plan (and, with `now`, what was done); whether it can go, or went.
pub fn run(ask: &Ask) -> Result<bool, String> {
    let text = std::fs::read_to_string(&ask.graph)
        .map_err(|e| format!("cannot read the graph file {}: {e}", ask.graph.display()))?;
    let graph = Graph::parse(&text).map_err(|e| format!("{}: {e}", ask.graph.display()))?;
    if ask.verb == Verb::Refresh {
        let rows = refresh(&graph, &GhCli).map_err(|e| e.to_string())?;
        let mut out = String::new();
        for (i, (fork, says)) in rows.iter().enumerate() {
            out.push_str(&format!("cutting.{n}.fork={fork}\ncutting.{n}.says={says}\n", n = i + 1));
        }
        crate::commands::status::write_report(&mut std::io::stdout().lock(), &out)?;
        return Ok(true);
    }
    let name = ask.target.clone().ok_or("this verb names a repository: owner/repo")?;
    let name = RepoName::try_from(name)?;
    let place = |fork: &RepoName| -> ClonePlace {
        let dir = ask.root.join(graph.clone_path(fork));
        ClonePlace { exists: dir.join(".git").exists(), dir }
    };
    let core = CoreAsk {
        upstream: ask.upstream.as_deref(),
        tracking: ask.tracking.as_deref(),
        upstream_branch: ask.upstream_branch.as_deref(),
        patches: ask.patches.as_deref(),
        org: ask.org.as_deref(),
    };
    let here = place(&name);
    let git = GitCli::new(&here.dir);
    let plan = match ask.verb {
        Verb::Import => plan_import(&graph, &GhCli, &name, &here, &core, &git),
        Verb::Add => plan_add(&graph, &GhCli, &name, &place, &core),
        Verb::Update => plan_update(&graph, &GhCli, &name, &here, &git),
        Verb::Remove => Ok(plan_remove(&graph, &name)),
        Verb::Refresh => unreachable!("refresh is answered above"),
    }
    .map_err(|e| e.to_string())?;
    let mut plan = plan;
    // the record is tried on the graph now: a conflict blocks the plan before any outward step
    let record = plan.steps.iter().find_map(|s| match s {
        rhizoid_core::cutting::Step::Record { record } => Some(record.clone()),
        _ => None,
    });
    if let Some(r) = record {
        if let Err(e) = rhizoid_core::cutting::record_fork(&text, &r) {
            plan.blocked.push(e);
        }
    }
    let mut out = render(&plan, ask.json);
    if !ask.now {
        crate::commands::status::write_report(&mut std::io::stdout().lock(), &out)?;
        return Ok(plan.blocked.is_empty());
    }
    if !plan.blocked.is_empty() {
        crate::commands::status::write_report(&mut std::io::stdout().lock(), &out)?;
        return Ok(false);
    }
    // the clone of the cutting that is worked on: for add, the fork it makes
    let module = RepoName::try_from(plan.module.clone())?;
    let at = place(&module);
    let git = GitCli::new(&at.dir);
    let home = crate::commands::assemble::rhizoid_home(&ask.worktrees, &at.dir)?;
    let doing = Doing { graph_file: &ask.graph, home: &home, today: &today() };
    let done = carry_out(&plan, &GhCli, &git, &Programs, &doing);
    let done_ok = done.is_ok();
    match done {
        Ok(lines) => {
            for (i, l) in lines.iter().enumerate() {
                out.push_str(&format!("done.{}={l}\n", i + 1));
            }
        }
        Err(e) => out.push_str(&format!("error={e}\n")),
    }
    crate::commands::status::write_report(&mut std::io::stdout().lock(), &out)?;
    if done_ok {
        Ok(true)
    } else {
        Err("the plan stopped: what was done is listed".to_string())
    }
}

fn render(plan: &Plan, json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(plan).expect("a plan serialises") + "\n";
    }
    plan.key_values().into_iter().map(|(k, v)| format!("{k}={v}\n")).collect()
}

/// Today as `YYYY-MM-DD` (UTC), from the clock.
fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    // days to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", if m <= 2 { y + 1 } else { y })
}
