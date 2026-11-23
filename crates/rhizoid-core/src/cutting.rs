//! Cuttings (rhizcut): a dependency taken as an owned fork - its tracking branch the upstream's, moved only by a
//! fast-forward; its changes on its patch side, which is one branch or the topic branches an `[[integration]]` edge
//! assembles. The graph file is its record: a `[[fork]]` edge with `patches` and `added` (decision
//! 2026-11-23-cuttings-live-in-the-graph).
//!
//! Every verb is a plan of steps, printed as it is; a step that writes to GitHub or to the fork (a fork created, a
//! branch pushed, merge-upstream) is marked outward, and nothing is done without `now`. GitHub is asked only through
//! [`GitHubPort`], git through [`GitPort`].

use crate::graph::{Fork, Graph, Remote, RepoName};
use crate::ports::{CommandPort, GitHubPort, GitPort, MergeOutcome, MergeResult, PortError};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The patch branch of a cutting that names none.
pub const PATCH_BRANCH: &str = "rhizoid/patch";

/// Where a cutting's changes live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatchSide {
    /// One branch, merged from tracking; `.rhizoid/module.toml` on it.
    Branch(String),
    /// The topic branches the repository's `[[integration]]` edge assembles: no patch branch.
    Integration,
}

impl PatchSide {
    /// What `patches` says (`None`: the default branch).
    pub fn of(patches: Option<&str>) -> PatchSide {
        match patches {
            Some("integration") => PatchSide::Integration,
            Some(b) => PatchSide::Branch(b.to_string()),
            None => PatchSide::Branch(PATCH_BRANCH.to_string()),
        }
    }

    fn word(&self) -> &str {
        match self {
            PatchSide::Branch(b) => b,
            PatchSide::Integration => "integration",
        }
    }
}

/// What a cutting's `[[fork]]` edge says, as a verb records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    /// The fork, `owner/repo`.
    pub fork: String,
    /// Its tracking branch (none for an integration cutting, which tracks releases).
    pub branch: Option<String>,
    /// Its upstream: `owner/repo` or a URL.
    pub upstream: String,
    /// The upstream's branch tracking follows.
    pub upstream_branch: Option<String>,
    /// For an integration cutting, whose releases its topics are on: the `[[integration]]` edge's `releases`.
    pub tracks: Option<String>,
    /// Its patch side.
    pub patches: String,
    /// `add` or `import`.
    pub added: String,
}

/// One step of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "step")]
pub enum Step {
    /// Fork `source` into `org` on GitHub.
    Fork { source: String, org: String },
    /// Clone the fork into `dir`.
    Clone { url: String, dir: String },
    /// Make the patch branch from tracking, with `.rhizoid/module.toml` on it (local).
    PatchBranch { branch: String, tracking: String, remote: String },
    /// The patch branch is on the fork already: adopted as it is.
    AdoptPatchBranch { branch: String },
    /// Push the patch branch to the fork.
    PushPatch { branch: String, remote: String },
    /// GitHub's merge-upstream: the fork's tracking branch fast-forwarded to its GitHub parent's.
    MergeUpstream { fork: String, branch: String },
    /// Fetch the upstream's branch and push it to the fork's tracking branch, only as a fast-forward.
    FastForwardTracking {
        upstream: String,
        upstream_branch: String,
        branch: String,
        remote: String,
    },
    /// Merge tracking into the patch branch in a fresh worktree, and push it.
    MergePatch { branch: String, tracking: String, remote: String },
    /// Write the cutting's record into the graph file (local).
    Record { record: Record },
    /// Take the cutting's `[[fork]]` edge out of the graph file (local; the fork stays on GitHub).
    Drop { fork: String },
}

impl Step {
    /// Whether it writes to GitHub or to the fork.
    pub fn outward(&self) -> bool {
        matches!(
            self,
            Step::Fork { .. }
                | Step::PushPatch { .. }
                | Step::MergeUpstream { .. }
                | Step::FastForwardTracking { .. }
                | Step::MergePatch { .. }
        )
    }

    /// The step in words.
    pub fn describe(&self) -> String {
        match self {
            Step::Fork { source, org } => format!("fork {source} into {org} on GitHub"),
            Step::Clone { url, dir } => format!("clone {url} into {dir}"),
            Step::PatchBranch { branch, tracking, remote } => {
                format!("make {branch} from the fork's {tracking} ({remote}), with .rhizoid/module.toml on it")
            }
            Step::AdoptPatchBranch { branch } => format!("adopt {branch}, on the fork already"),
            Step::PushPatch { branch, remote } => format!("push {branch} to the fork ({remote})"),
            Step::MergeUpstream { fork, branch } => {
                format!("merge-upstream {fork} {branch}: fast-forward it to its GitHub parent's")
            }
            Step::FastForwardTracking { upstream, upstream_branch, branch, remote } => format!(
                "fetch {upstream} {upstream_branch} and push it to the fork's {branch} ({remote}), as a fast-forward only"
            ),
            Step::MergePatch { branch, tracking, remote } => {
                format!("merge {tracking} into {branch} in a fresh worktree, and push {branch} to the fork ({remote})")
            }
            Step::Record { record } => {
                let tracking = match (&record.tracks, &record.branch) {
                    (Some(t), _) => format!("tracking the releases of {t}"),
                    (None, Some(b)) => format!("tracking {b} from {} {}", record.upstream, record.upstream_branch.as_deref().unwrap_or(b)),
                    (None, None) => format!("from {}", record.upstream),
                };
                format!(
                    "record in the graph: fork {} {tracking} (upstream {}), patches {}, added by {}",
                    record.fork, record.upstream, record.patches, record.added
                )
            }
            Step::Drop { fork } => format!("take fork {fork} out of the graph (the fork itself stays)"),
        }
    }
}

/// A verb's plan: its steps, in order, and what stops it from being done now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// The verb.
    pub verb: &'static str,
    /// The cutting, `owner/repo`.
    pub module: String,
    /// The steps.
    pub steps: Vec<Step>,
    /// Why it cannot be done now; empty: it can.
    pub blocked: Vec<String>,
    /// Facts read on the way, for the person reading the plan.
    pub notes: Vec<String>,
}

impl Plan {
    fn new(verb: &'static str, module: &str) -> Plan {
        Plan {
            verb,
            module: module.to_string(),
            steps: Vec::new(),
            blocked: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Every field as `key=value` pairs.
    pub fn key_values(&self) -> Vec<(String, String)> {
        let mut kv = vec![
            ("verb".to_string(), self.verb.to_string()),
            ("module".to_string(), self.module.clone()),
        ];
        for (i, s) in self.steps.iter().enumerate() {
            kv.push((format!("step.{}", i + 1), s.describe()));
            kv.push((
                format!("step.{}.outward", i + 1),
                if s.outward() { "yes" } else { "no" }.to_string(),
            ));
        }
        for (i, n) in self.notes.iter().enumerate() {
            kv.push((format!("note.{}", i + 1), n.clone()));
        }
        for (i, b) in self.blocked.iter().enumerate() {
            kv.push((format!("blocked.{}", i + 1), b.clone()));
        }
        kv
    }
}

/// The fork's local clone, where the graph says it is.
pub struct ClonePlace {
    /// Its folder.
    pub dir: PathBuf,
    /// Whether it is there.
    pub exists: bool,
}

/// What import and add are asked beyond the fork.
#[derive(Default)]
pub struct Ask<'a> {
    /// The upstream, when GitHub's parent is not it (`owner/repo` or a URL).
    pub upstream: Option<&'a str>,
    /// The tracking branch (default: the fork's default branch, or what the graph says).
    pub tracking: Option<&'a str>,
    /// The upstream's branch (default: the tracking branch's name).
    pub upstream_branch: Option<&'a str>,
    /// The patch side: a branch or `integration`.
    pub patches: Option<&'a str>,
    /// The org to fork into (add).
    pub org: Option<&'a str>,
}

fn edge<'g>(graph: &'g Graph, fork: &RepoName) -> Option<&'g Fork> {
    graph.forks.iter().find(|f| matches!(&f.fork, Remote::GitHub(n) if n == fork))
}

fn has_integration(graph: &Graph, fork: &RepoName) -> bool {
    graph.integrations.iter().any(|i| &i.repo == fork)
}

/// The plan of `import FORK`: an existing fork adopted as a cutting.
pub fn plan_import(
    graph: &Graph,
    gh: &impl GitHubPort,
    fork: &RepoName,
    clone: &ClonePlace,
    ask: &Ask,
    git: &impl GitPort,
) -> Result<Plan, PortError> {
    let mut plan = Plan::new("import", fork.as_str());
    let had = edge(graph, fork);
    if !gh.repo_exists(fork.as_str())? {
        plan.blocked.push(format!("{fork} is not on GitHub (or not visible to gh)"));
        return Ok(plan);
    }
    let parent = gh.parent(fork.as_str())?;
    let upstream = ask
        .upstream
        .map(str::to_string)
        .or_else(|| had.map(|f| f.upstream.to_string()))
        .or_else(|| parent.clone());
    match (&parent, &upstream) {
        (Some(p), Some(u)) if p != u => {
            plan.notes.push(format!("GitHub says {fork} is a fork of {p}; its upstream is {u}"))
        }
        (None, Some(u)) => plan.notes.push(format!(
            "{fork} is no GitHub fork; its upstream is {u}, followed by fetch and push"
        )),
        _ => {}
    }
    let Some(upstream) = upstream else {
        plan.blocked.push(format!(
            "{fork} names no upstream: GitHub has no parent for it - give --upstream"
        ));
        return Ok(plan);
    };
    let tracking =
        match ask.tracking.map(str::to_string).or_else(|| had.and_then(|f| f.branch.clone())) {
            Some(t) => t,
            None => gh.default_branch(fork.as_str())?,
        };
    let upstream_branch = ask
        .upstream_branch
        .map(str::to_string)
        .or_else(|| had.and_then(|f| f.upstream_branch.clone()))
        .unwrap_or_else(|| tracking.clone());
    let side = PatchSide::of(ask.patches.or_else(|| had.and_then(|f| f.patches.as_deref())));
    if let (Some(h), Some(p)) = (had.and_then(|f| f.patches.as_deref()), ask.patches) {
        if h != p {
            plan.blocked.push(format!(
                "the graph says patches = {h:?}; --patches says {p:?}: change the graph by hand"
            ));
        }
    }
    // an integration cutting's topics are on the releases of the integration's source, not on a branch of the fork:
    // that is what it tracks, unless --tracked-ref (or the graph) names a branch
    let named_branch = ask.tracking.is_some() || had.and_then(|f| f.branch.as_ref()).is_some();
    let (branch, upstream_branch, tracks) = match &side {
        PatchSide::Integration if !named_branch => {
            let releases = graph
                .integrations
                .iter()
                .find(|i| &i.repo == fork)
                .and_then(|i| i.releases.as_ref().map(|r| r.to_string()));
            match releases {
                Some(r) => {
                    plan.notes.push(format!(
                        "{fork}'s default branch on GitHub is {tracking}: information only - its topics follow the releases of {r}"
                    ));
                    (None, None, Some(r))
                }
                None => {
                    if has_integration(graph, fork) {
                        plan.blocked.push(format!(
                            "the [[integration]] edge for {fork} names no releases source: add releases = \"owner/repo\" to it, or give --tracked-ref"
                        ));
                    }
                    (None, None, None)
                }
            }
        }
        _ => (Some(tracking.clone()), Some(upstream_branch), None),
    };
    let remote = if clone.exists { fork_remote(git, fork)? } else { None };
    let on_fork = match (&side, &remote) {
        (PatchSide::Branch(b), Some(r)) => git.commit(&format!("refs/remotes/{r}/{b}"))?.is_some(),
        _ => false,
    };
    patch_steps(&mut plan, graph, fork, &side, &tracking, clone, remote.as_deref(), on_fork, false);
    plan.steps.push(Step::Record {
        record: Record {
            fork: fork.to_string(),
            branch,
            upstream,
            upstream_branch,
            tracks,
            patches: side.word().to_string(),
            added: had.and_then(|f| f.added.clone()).unwrap_or_else(|| "import".to_string()),
        },
    });
    Ok(plan)
}

/// The plan of `add SOURCE`: a fork made on GitHub, taken as a cutting.
pub fn plan_add(
    graph: &Graph,
    gh: &impl GitHubPort,
    source: &RepoName,
    clone: &dyn Fn(&RepoName) -> ClonePlace,
    ask: &Ask,
) -> Result<Plan, PortError> {
    let mut plan = Plan::new("add", source.as_str());
    let Some(org) = ask.org.map(str::to_string).or_else(|| graph.fork_org.clone()) else {
        plan.blocked.push("no org to fork into: give --org, or fork_org in the graph".to_string());
        return Ok(plan);
    };
    if !gh.repo_exists(source.as_str())? {
        plan.blocked.push(format!("{source} is not on GitHub (or not visible to gh)"));
        return Ok(plan);
    }
    let name = source.as_str().rsplit('/').next().unwrap_or(source.as_str());
    let fork = RepoName::try_from(format!("{org}/{name}")).map_err(PortError)?;
    plan.module = fork.to_string();
    if gh.repo_exists(fork.as_str())? {
        plan.blocked.push(format!("{fork} exists already: import it"));
        return Ok(plan);
    }
    let tracking = match ask.tracking {
        Some(t) => t.to_string(),
        None => gh.default_branch(source.as_str())?,
    };
    plan.steps.push(Step::Fork { source: source.to_string(), org: org.clone() });
    let side = PatchSide::of(ask.patches);
    let place = clone(&fork);
    patch_steps(&mut plan, graph, &fork, &side, &tracking, &place, None, false, true);
    plan.steps.push(Step::Record {
        record: Record {
            fork: fork.to_string(),
            branch: Some(tracking.clone()),
            upstream: source.to_string(),
            upstream_branch: Some(ask.upstream_branch.unwrap_or(&tracking).to_string()),
            tracks: None,
            patches: side.word().to_string(),
            added: "add".to_string(),
        },
    });
    Ok(plan)
}

/// The steps that make a cutting's patch side, or what blocks them.
#[allow(clippy::too_many_arguments)]
fn patch_steps(
    plan: &mut Plan,
    graph: &Graph,
    fork: &RepoName,
    side: &PatchSide,
    tracking: &str,
    clone: &ClonePlace,
    remote: Option<&str>,
    on_fork: bool,
    new_fork: bool,
) {
    match side {
        PatchSide::Integration => {
            if !has_integration(graph, fork) {
                plan.blocked.push(format!(
                    "no [[integration]] edge names {fork}: add one to the graph first - its topics are this cutting's patch side"
                ));
            }
            plan.notes.push(
                "patches = integration: no patch branch is made and nothing is pushed".to_string(),
            );
        }
        PatchSide::Branch(branch) => {
            if new_fork && !clone.exists {
                plan.steps.push(Step::Clone {
                    url: format!("https://github.com/{fork}"),
                    dir: clone.dir.display().to_string(),
                });
            } else if !clone.exists {
                plan.blocked.push(format!(
                    "no clone of {fork} at {}: clone it (git clone https://github.com/{fork} {0}) or name its folder in [clones]",
                    clone.dir.display()
                ));
                return;
            }
            // a clone of a new fork names it origin; an existing clone names it however it does
            let remote = match (new_fork, remote) {
                (true, _) => "origin".to_string(),
                (false, Some(r)) => r.to_string(),
                (false, None) => {
                    plan.blocked.push(format!(
                        "the clone at {} has no remote for github.com/{fork}: add one (git remote add fork https://github.com/{fork})",
                        clone.dir.display()
                    ));
                    return;
                }
            };
            if on_fork {
                plan.steps.push(Step::AdoptPatchBranch { branch: branch.clone() });
            } else {
                plan.steps.push(Step::PatchBranch {
                    branch: branch.clone(),
                    tracking: tracking.to_string(),
                    remote: remote.clone(),
                });
                plan.steps.push(Step::PushPatch { branch: branch.clone(), remote });
            }
        }
    }
}

/// The plan of `update MODULE`: tracking fast-forwarded to the upstream, the patch branch merged from it.
pub fn plan_update(
    graph: &Graph,
    gh: &impl GitHubPort,
    fork: &RepoName,
    clone: &ClonePlace,
    git: &impl GitPort,
) -> Result<Plan, PortError> {
    let mut plan = Plan::new("update", fork.as_str());
    let Some(f) = edge(graph, fork).filter(|f| f.added.is_some()) else {
        plan.blocked.push(format!("{fork} is no cutting in the graph: import it first"));
        return Ok(plan);
    };
    // an integration cutting that tracks releases has no branch to move: its topics are rebased onto releases, by hand
    if PatchSide::of(f.patches.as_deref()) == PatchSide::Integration && f.branch.is_none() {
        let tracks = f.tracks.clone().unwrap_or_else(|| "its integration's releases".to_string());
        plan.notes.push(format!(
            "patches = integration, tracking the releases of {tracks}: nothing is merged or pushed - a topic behind the newest release is rebased onto it, the lane's work"
        ));
        return Ok(plan);
    }
    let tracking = match &f.branch {
        Some(b) => b.clone(),
        None => gh.default_branch(fork.as_str())?,
    };
    let upstream_branch = f.upstream_branch.clone().unwrap_or_else(|| tracking.clone());
    // merge-upstream merges the same-named branch of the GitHub parent: only then is it the way
    let github_parent = match &f.upstream {
        Remote::GitHub(u) => {
            upstream_branch == tracking && gh.parent(fork.as_str())?.as_deref() == Some(u.as_str())
        }
        Remote::Url(_) => false,
    };
    let side = PatchSide::of(f.patches.as_deref());
    let needs_clone = !github_parent || matches!(side, PatchSide::Branch(_));
    let remote = if clone.exists { fork_remote(git, fork)? } else { None };
    if needs_clone && !clone.exists {
        plan.blocked.push(format!(
            "no clone of {fork} at {}: clone it, or name its folder in [clones]",
            clone.dir.display()
        ));
    } else if needs_clone && remote.is_none() {
        plan.blocked.push(format!(
            "the clone at {} has no remote for github.com/{fork}",
            clone.dir.display()
        ));
    }
    let remote = remote.unwrap_or_else(|| "origin".to_string());
    if github_parent {
        plan.steps.push(Step::MergeUpstream { fork: fork.to_string(), branch: tracking.clone() });
    } else {
        plan.steps.push(Step::FastForwardTracking {
            upstream: f.upstream.url(),
            upstream_branch,
            branch: tracking.clone(),
            remote: remote.clone(),
        });
    }
    match side {
        PatchSide::Branch(b) => plan.steps.push(Step::MergePatch { branch: b, tracking, remote }),
        PatchSide::Integration => plan.notes.push(
            "patches = integration: its topics are assembled by rhizoid assemble, not merged here"
                .to_string(),
        ),
    }
    Ok(plan)
}

/// The clone's remote that is the fork on GitHub (its URL ends `github.com/OWNER/REPO`), if one is.
pub fn fork_remote(git: &impl GitPort, fork: &RepoName) -> Result<Option<String>, PortError> {
    let want = fork.as_str().to_ascii_lowercase();
    Ok(git.remotes()?.into_iter().find_map(|(name, url)| {
        let url = url.trim_end_matches('/').trim_end_matches(".git").to_ascii_lowercase();
        (url.ends_with(&format!("github.com/{want}"))
            || url.ends_with(&format!("github.com:{want}")))
        .then_some(name)
    }))
}

/// The plan of `remove MODULE`: the cutting's edge out of the graph; the fork itself never deleted.
pub fn plan_remove(graph: &Graph, fork: &RepoName) -> Plan {
    let mut plan = Plan::new("remove", fork.as_str());
    match edge(graph, fork) {
        Some(_) => plan.steps.push(Step::Drop { fork: fork.to_string() }),
        None => plan.blocked.push(format!("no [[fork]] edge names {fork}")),
    }
    plan.notes
        .push("the fork stays on GitHub: deleting a repository is a person's act".to_string());
    plan
}

/// What `refresh` reads of each cutting: the graph's record against GitHub and the clone. Read-only.
pub fn refresh(graph: &Graph, gh: &impl GitHubPort) -> Result<Vec<(String, String)>, PortError> {
    let mut out = Vec::new();
    for f in &graph.forks {
        let Remote::GitHub(fork) = &f.fork else { continue };
        let kind = match &f.added {
            Some(a) => format!(
                "cutting, added by {a}, patches {}",
                PatchSide::of(f.patches.as_deref()).word()
            ),
            None => "fork edge, not a cutting".to_string(),
        };
        let exists = gh.repo_exists(fork.as_str())?;
        let mut says = vec![kind];
        if !exists {
            says.push("not on GitHub".to_string());
        } else if let (Remote::GitHub(u), Some(p)) = (&f.upstream, gh.parent(fork.as_str())?) {
            if p != u.as_str() {
                says.push(format!("GitHub's parent is {p}, the graph's upstream {u}"));
            }
        }
        if f.patches.as_deref() == Some("integration") && !has_integration(graph, fork) {
            says.push("patches = integration, and no [[integration]] edge names it".to_string());
        }
        out.push((fork.to_string(), says.join("; ")));
    }
    Ok(out)
}

/// What carrying out a plan needs.
pub struct Doing<'a> {
    /// The graph file.
    pub graph_file: &'a Path,
    /// rhizoid's worktree home for the fork's clone.
    pub home: &'a Path,
    /// The date the cutting is attached on (`YYYY-MM-DD`), for `.rhizoid/module.toml`.
    pub today: &'a str,
}

/// Carry out a plan, step by step; refused when it is blocked. What was done, in words.
pub fn carry_out(
    plan: &Plan,
    gh: &impl GitHubPort,
    git: &impl GitPort,
    cmd: &impl CommandPort,
    doing: &Doing,
) -> Result<Vec<String>, PortError> {
    if !plan.blocked.is_empty() {
        return Err(PortError(format!("not done: {}", plan.blocked.join("; "))));
    }
    let mut done = Vec::new();
    let record = plan.steps.iter().find_map(|s| match s {
        Step::Record { record } => Some(record.clone()),
        _ => None,
    });
    for step in &plan.steps {
        match step {
            Step::Fork { source, org } => {
                gh.fork(source, org)?;
            }
            Step::Clone { url, dir } => {
                let parent = Path::new(dir).parent().unwrap_or(Path::new("/"));
                std::fs::create_dir_all(parent)
                    .map_err(|e| PortError(format!("{}: {e}", parent.display())))?;
                let argv: Vec<String> =
                    ["git", "clone", "-q", url, dir].iter().map(|s| s.to_string()).collect();
                let (exit, _, err) = cmd.run(&argv, parent)?;
                if exit != Some(0) {
                    return Err(PortError(format!(
                        "git clone {url}: {} - the fork is made; once it is ready, rhizoid import with now takes it on",
                        err.trim()
                    )));
                }
                // a fork GitHub has only just made may not be ready to clone at once: the error says to import it
            }
            Step::PatchBranch { branch, tracking, remote } => {
                // a patch branch an earlier run made here, whose push failed, is taken as it is
                if git.commit(&format!("refs/heads/{branch}"))?.is_some() {
                    if git.show(&format!("refs/heads/{branch}"), ".rhizoid/module.toml")?.is_none()
                    {
                        return Err(PortError(format!("a local {branch} without .rhizoid/module.toml is there: someone's branch? Move it away")));
                    }
                    done.push(format!("{branch} made here before, taken as it is"));
                    continue;
                }
                let at = git.fetch_ref(remote, tracking)?;
                let worktree = doing.home.join(branch.replace('/', "-"));
                let _ = git.remove_worktree(&worktree);
                git.add_branch_worktree(&worktree, branch, &at)?;
                let made =
                    module_file(&worktree, record.as_ref(), &at, doing.today).and_then(|()| {
                        git.commit_files(
                            &worktree,
                            &[".rhizoid/module.toml".to_string()],
                            "chore: rhizoid: this fork is a cutting (.rhizoid/module.toml)",
                        )
                    });
                let _ = git.remove_worktree(&worktree);
                if let Err(e) = made {
                    let _ = git.delete_branch(branch);
                    return Err(e);
                }
            }
            Step::AdoptPatchBranch { .. } => {}
            Step::PushPatch { branch, remote } => {
                let at = git
                    .commit(&format!("refs/heads/{branch}"))?
                    .ok_or_else(|| PortError(format!("no {branch} to push")))?;
                git.push_ref(&at, remote, branch)?;
            }
            Step::MergeUpstream { fork, branch } => {
                if let MergeOutcome::FastForwarded { new_commit } =
                    gh.merge_upstream(fork, branch)?
                {
                    done.push(format!("{fork} {branch} is now {new_commit}"));
                }
            }
            Step::FastForwardTracking { upstream, upstream_branch, branch, remote } => {
                let theirs = git.fetch_ref(upstream, upstream_branch)?;
                let ours = git.fetch_ref(remote, branch)?;
                if ours != theirs {
                    if !git.is_ancestor(&ours, &theirs)? {
                        return Err(PortError(format!(
                            "the fork's {branch} is not behind {upstream} {upstream_branch} alone: it was changed by hand - nothing pushed"
                        )));
                    }
                    git.push_ref(&theirs, remote, branch)?;
                }
            }
            Step::MergePatch { branch, tracking, remote } => {
                let tip = git.fetch_ref(remote, tracking)?;
                let patch = git.fetch_ref(remote, branch)?;
                let worktree = doing.home.join(format!("{}-update", branch.replace('/', "-")));
                git.add_worktree(&worktree, &patch)?;
                let merged = git.merge(
                    &worktree,
                    &tip,
                    &format!("rhizoid update: merge {tracking} into {branch}"),
                );
                let head = git.head(&worktree);
                let _ = git.remove_worktree(&worktree);
                match merged? {
                    MergeResult::Conflict { paths } => {
                        return Err(PortError(format!(
                            "merging {tracking} into {branch} conflicts in {}: nothing pushed",
                            paths.join(", ")
                        )))
                    }
                    MergeResult::AlreadyContained => {}
                    MergeResult::Merged { .. } => git.push_ref(&head?, remote, branch)?,
                }
            }
            Step::Record { record } => {
                let text = std::fs::read_to_string(doing.graph_file)
                    .map_err(|e| PortError(format!("{}: {e}", doing.graph_file.display())))?;
                let written = record_fork(&text, record).map_err(PortError)?;
                std::fs::write(doing.graph_file, written)
                    .map_err(|e| PortError(format!("{}: {e}", doing.graph_file.display())))?;
            }
            Step::Drop { fork } => {
                let text = std::fs::read_to_string(doing.graph_file)
                    .map_err(|e| PortError(format!("{}: {e}", doing.graph_file.display())))?;
                let written = drop_fork(&text, fork).map_err(PortError)?;
                std::fs::write(doing.graph_file, written)
                    .map_err(|e| PortError(format!("{}: {e}", doing.graph_file.display())))?;
            }
        }
        done.push(step.describe());
    }
    Ok(done)
}

/// `.rhizoid/module.toml` on the patch branch: where the cutting came from, written on the fork itself.
fn module_file(
    worktree: &Path,
    record: Option<&Record>,
    synced: &str,
    today: &str,
) -> Result<(), PortError> {
    let r = record
        .ok_or_else(|| PortError("a patch branch is made with the cutting's record".to_string()))?;
    let added = if r.added == "add" { "created" } else { "imported" };
    let text = format!(
        "# This fork is a rhizoid cutting: its tracking branch is {upstream}'s, its changes are on this branch.\n\
         origin = \"{upstream}\"\ntracked_ref = \"{branch}\"\nlast_synced_commit = \"{synced}\"\nadded_as = \"{added}\"\nattached_on = \"{today}\"\n",
        upstream = r.upstream,
        branch = r.branch.as_deref().unwrap_or(""),
    );
    let dir = worktree.join(".rhizoid");
    std::fs::create_dir_all(&dir).map_err(|e| PortError(format!("{}: {e}", dir.display())))?;
    std::fs::write(dir.join("module.toml"), text)
        .map_err(|e| PortError(format!("{}: {e}", dir.display())))
}

/// A TOML line's key and plain value: either kind of quotes, any spacing, an inline comment left out.
fn key_value(line: &str) -> Option<(&str, String)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || key.starts_with('#') || key.starts_with('[') {
        return None;
    }
    let rest = rest.trim();
    let value = match rest.chars().next() {
        Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or("").to_string(),
        _ => rest.split('#').next().unwrap_or("").trim().to_string(),
    };
    Some((key, value))
}

/// Every `[[fork]]` table whose `fork` is `fork`: its first line and one past its last own line (the comments and
/// blank lines before the next table belong to what follows).
fn fork_blocks(lines: &[&str], fork: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() != "[[fork]]" {
            i += 1;
            continue;
        }
        let next = (i + 1..lines.len())
            .find(|&j| lines[j].trim_start().starts_with('['))
            .unwrap_or(lines.len());
        let mut end = next;
        while end > i + 1
            && (lines[end - 1].trim().is_empty() || lines[end - 1].trim_start().starts_with('#'))
        {
            end -= 1;
        }
        if lines[i + 1..end]
            .iter()
            .any(|l| key_value(l).is_some_and(|(k, v)| k == "fork" && v == fork))
        {
            found.push((i, end));
        }
        i = next;
    }
    found
}

/// A text's lines and the line ending it uses.
fn split(text: &str) -> (Vec<&str>, &'static str) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    (text.lines().collect(), eol)
}

/// The graph's text with the cutting's record in its `[[fork]]` edge: missing keys added after its `fork` line, a
/// new edge appended when there is none; a key it already says otherwise, or a fork named twice, is refused. The
/// rest of the file is kept as it is; the result is read back before it is given.
pub fn record_fork(text: &str, r: &Record) -> Result<String, String> {
    let (lines, eol) = split(text);
    let fields: Vec<(&str, &str)> = [
        ("branch", r.branch.as_deref()),
        ("upstream", Some(r.upstream.as_str())),
        ("upstream_branch", r.upstream_branch.as_deref()),
        ("tracks", r.tracks.as_deref()),
        ("patches", Some(r.patches.as_str())),
        ("added", Some(r.added.as_str())),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|v| (k, v)))
    .collect();
    let blocks = fork_blocks(&lines, &r.fork);
    let out = match blocks.as_slice() {
        [] => {
            let mut out = text.trim_end_matches(['\n', '\r']).to_string();
            for line in [
                String::new(),
                format!("# a cutting (rhizcut) of {}; its changes on {}", r.upstream, r.patches),
                "[[fork]]".to_string(),
                format!("fork = \"{}\"", r.fork),
            ] {
                out.push_str(eol);
                out.push_str(&line);
            }
            for (k, v) in &fields {
                out.push_str(&format!("{eol}{k} = \"{v}\""));
            }
            out + eol
        }
        [(start, end)] => {
            let mut add = Vec::new();
            for (k, v) in &fields {
                let said = lines[start + 1..*end]
                    .iter()
                    .find_map(|l| key_value(l).filter(|(key, _)| key == k).map(|(_, val)| val));
                match said {
                    Some(s) if s == *v => {}
                    Some(s) => {
                        return Err(format!(
                            "the graph's fork {} says {k} = {s:?}, not {v:?}: change it by hand",
                            r.fork
                        ))
                    }
                    None => add.push(format!("{k} = \"{v}\"")),
                }
            }
            let at = (start + 1..*end)
                .find(|&j| key_value(lines[j]).is_some_and(|(k, _)| k == "fork"))
                .expect("the block has its fork line");
            let mut out: Vec<String> = lines[..=at].iter().map(|s| s.to_string()).collect();
            out.extend(add);
            out.extend(lines[at + 1..].iter().map(|s| s.to_string()));
            out.join(eol) + eol
        }
        _ => {
            return Err(format!(
                "the graph names fork {} more than once: make it one edge by hand",
                r.fork
            ))
        }
    };
    let graph =
        Graph::parse(&out).map_err(|e| format!("the graph would not read after the edit: {e}"))?;
    let fork = RepoName::try_from(r.fork.clone())?;
    match edge(&graph, &fork) {
        Some(f)
            if f.patches.as_deref() == Some(r.patches.as_str())
                && f.added.as_deref() == Some(r.added.as_str()) =>
        {
            Ok(out)
        }
        _ => Err(format!("the graph did not read back the cutting {}", r.fork)),
    }
}

/// The graph's text without the `[[fork]]` edge of `fork` (and the comment rhizcut wrote above it).
pub fn drop_fork(text: &str, fork: &str) -> Result<String, String> {
    let (lines, eol) = split(text);
    let (start, end) = match fork_blocks(&lines, fork).as_slice() {
        [] => return Err(format!("no [[fork]] edge names {fork}")),
        [one] => *one,
        _ => {
            return Err(format!(
                "the graph names fork {fork} more than once: make it one edge by hand"
            ))
        }
    };
    let start = if start > 0 && lines[start - 1].starts_with("# a cutting (rhizcut):") {
        start - 1
    } else {
        start
    };
    let mut out: Vec<&str> = lines[..start].to_vec();
    out.extend(&lines[end..]);
    let out = out.join(eol) + eol;
    Graph::parse(&out).map_err(|e| format!("the graph would not read after the edit: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAPH: &str = "rev = \"origin/main\"\n\n[[fork]]\nfork = \"grenudi/linux\"\nupstream = \"sm7150-mainline/linux\"\n\n[[tool]]\nname = \"t\"\nsource = \"a/b\"\ntag_prefix = \"v\"\npin = \"p\"\nkey = \"k\"\nfollowers = []\n";

    fn record(patches: &str) -> Record {
        Record {
            fork: "grenudi/linux".into(),
            branch: Some("master".into()),
            upstream: "sm7150-mainline/linux".into(),
            upstream_branch: Some("master".into()),
            tracks: None,
            patches: patches.into(),
            added: "import".into(),
        }
    }

    #[test]
    fn a_record_fills_the_existing_edge_and_keeps_the_rest_of_the_file() {
        let out = record_fork(GRAPH, &record("integration")).unwrap();
        assert!(out.contains("fork = \"grenudi/linux\"\nbranch = \"master\"\nupstream_branch = \"master\"\npatches = \"integration\"\nadded = \"import\"\nupstream = \"sm7150-mainline/linux\""), "{out}");
        assert!(out.contains("[[tool]]\nname = \"t\""));
        assert!(
            record_fork(&out, &record("integration")).unwrap() == out,
            "recording twice changes nothing"
        );
        assert!(record_fork(&out, &record("rhizoid/patch"))
            .unwrap_err()
            .contains("change it by hand"));
    }

    #[test]
    fn a_new_cutting_is_appended_and_a_dropped_one_goes() {
        let mut r = record("rhizoid/patch");
        r.fork = "grenudi/other".into();
        let out = record_fork(GRAPH, &r).unwrap();
        let g = Graph::parse(&out).unwrap();
        assert_eq!(g.forks.len(), 2);
        let back = drop_fork(&out, "grenudi/other").unwrap();
        assert_eq!(Graph::parse(&back).unwrap().forks.len(), 1);
        assert!(drop_fork(GRAPH, "nobody/here").is_err());
    }

    #[test]
    fn the_edit_keeps_line_endings_quotes_and_the_comments_of_what_follows() {
        let crlf = "[[fork]]\r\nfork = 'grenudi/linux'  # the kernel\r\nupstream = \"sm7150-mainline/linux\"\r\n\r\n# the next thing\r\n[[tool]]\r\nname = \"t\"\r\nsource = \"a/b\"\r\ntag_prefix = \"v\"\r\npin = \"p\"\r\nkey = \"k\"\r\nfollowers = []\r\n";
        let out = record_fork(crlf, &record("integration")).unwrap();
        assert!(!out.replace("\r\n", "").contains('\n'), "CRLF kept: {out:?}");
        let dropped = drop_fork(&out, "grenudi/linux").unwrap();
        assert!(dropped.contains("# the next thing\r\n[[tool]]"), "{dropped:?}");
        let twice = format!("{GRAPH}\n[[fork]]\nfork = \"grenudi/linux\"\nupstream = \"x/y\"\n");
        assert!(record_fork(&twice, &record("integration"))
            .unwrap_err()
            .contains("more than once"));
        assert!(drop_fork(&twice, "grenudi/linux").unwrap_err().contains("more than once"));
    }

    #[test]
    fn a_patch_side_is_a_branch_or_the_integration() {
        assert_eq!(PatchSide::of(None), PatchSide::Branch(PATCH_BRANCH.into()));
        assert_eq!(PatchSide::of(Some("integration")), PatchSide::Integration);
        assert_eq!(PatchSide::of(Some("mine")), PatchSide::Branch("mine".into()));
        assert!(Graph::parse("[[fork]]\nfork = \"a/b\"\nupstream = \"c/d\"\nadded = \"stolen\"\n")
            .is_err());
    }
}
