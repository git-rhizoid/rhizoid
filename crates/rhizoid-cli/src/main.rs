//! `rhizoid` - fork, own, and sync git dependencies instead of relying on a
//! package manager. The invocation surface is declared once, in `cli.rs`
//! (argenv); this file reads it and runs the command.
//!
//! Exit status: 0 done; 1 `status` found drift or a command failed; 2 the
//! invocation is wrong or `status` could not read the world.

mod cli;
mod commands;
mod gh_cli;
mod git_cli;
mod git_repos;

use argenv_resolve::generate::{self, Program, Shell};
use argenv_resolve::{lint, InputResolveExt, LintPolicy, Resolution, Severity};
use cli::{Command, Contract};
use commands::status::Ask;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAN_DATE: &str = "2026-11-23";
const ABOUT: &str = "Fork, own and sync git dependencies, report every edge of a repository graph (rhizoid status) and whether this version can read it (rhizoid check), merge a named list of branches onto a base into a tag (rhizoid assemble), draft the next series version of a branch for its upstream (rhizoid send-back), keep every pin in one place (rhizoid bump), and lay a slice as idealkb's follow.sh does (rhizoid follow)";

const COMMANDS: &[generate::Command<'static>] = &[
    generate::Command { name: "add", summary: "Fork a repository on GitHub and take it as a cutting (plan; now does it)" },
    generate::Command { name: "import", summary: "Take an existing fork as a cutting (plan; now does it)" },
    generate::Command { name: "update", summary: "Fast-forward a cutting's tracking branch and merge it into its patch branch (plan; now does it)" },
    generate::Command { name: "refresh", summary: "Read each cutting of the graph against GitHub; change nothing" },
    generate::Command { name: "remove", summary: "Take a cutting out of the graph; the fork stays (plan; now does it)" },
    generate::Command {
        name: "status",
        summary: "Every edge of a repository graph: current, behind or moved by hand, and how",
    },
    generate::Command {
        name: "check",
        summary: "Whether this rhizoid can read the graph: every edge parsed and checked, no clones, no network",
    },
    generate::Command {
        name: "assemble",
        summary: "Merge a named list of branches onto its base in a fresh worktree, and tag it int/DEVICE/LIST/BASE/N",
    },
    generate::Command {
        name: "send-back",
        summary: "Draft the next series version of a branch for its upstream: rebased, cut, range-diffed, checked, tagged as a draft",
    },
    generate::Command {
        name: "bump",
        summary: "Every pin of a graph in one place; with now, bump each pin behind that names rhizoid-bump, on a branch",
    },
    generate::Command {
        name: "follow",
        summary: "Lay a release of a source into a base, as ideal-knowledge-base's follow.sh does",
    },
    generate::Command { name: "completions", summary: "Shell completions for rhizoid" },
    generate::Command { name: "man", summary: "The manual page, in roff" },
];

fn program() -> Program<'static> {
    Program {
        name: "rhizoid",
        version: VERSION,
        about: ABOUT,
        commands: COMMANDS,
        exit_status: &[
            ("0", "done; for status, no edge is drift; for assemble, tagged (and equal to --reproduces); for send-back, the version tagged"),
            ("1", "status found drift; assemble stopped at a conflict or differs from --reproduces; send-back stopped at a conflict or a check failed; or the command failed"),
            ("2", "the invocation is wrong, or status or assemble could not read or change the repositories it was asked about"),
        ],
        see_also: &["git(1)", "gh(1)"],
        date: MAN_DATE,
        asked: &[],
    }
}

/// Resolve argv and the environment; a finding that is an error ends the program with status 2.
fn resolve() -> Resolution {
    let records = Contract::records();
    let resolved = argenv_resolve::parse(&records);
    let findings = lint(&records, &resolved);
    let policy = LintPolicy::default();
    for f in &findings {
        let label = if policy.severity(f) == Severity::Error { "error" } else { "warning" };
        eprintln!("rhizoid: {label}: {f}");
    }
    if findings.iter().any(|f| policy.severity(f) == Severity::Error) {
        std::process::exit(2);
    }
    resolved
}

/// What the person asked, parsed once: the command with everything it needs, already decided.
enum Request {
    Help,
    Version,
    Cutting(commands::cutting::Ask),
    Status(Ask),
    Check { graph: PathBuf },
    Assemble(commands::assemble::Ask),
    AssembleDone { tag: String, repo: PathBuf, force: bool },
    SendBack(commands::send_back::Ask),
    Bump(commands::bump::Ask),
    Follow(commands::follow::Ask),
    Completions(Shell),
    Man,
}

fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("cannot read the current folder: {e}"))
}

fn request(r: &Resolution) -> Result<Request, String> {
    if Contract::HELP.get_from(r).unwrap_or(false) {
        return Ok(Request::Help);
    }
    if Contract::VERSION.get_from(r).unwrap_or(false) {
        return Ok(Request::Version);
    }
    let Some(command) = Contract::COMMAND.get_from(r) else {
        return Ok(Request::Help);
    };
    let target = Contract::TARGET.get_from(r);
    Ok(match command {
        Command::Add | Command::Import | Command::Update | Command::Refresh | Command::Remove => {
            use commands::cutting::Verb;
            let verb = match command {
                Command::Add => Verb::Add,
                Command::Import => Verb::Import,
                Command::Update => Verb::Update,
                Command::Refresh => Verb::Refresh,
                _ => Verb::Remove,
            };
            Request::Cutting(commands::cutting::Ask {
                verb,
                target,
                now: Contract::NOW.get_from(r).as_deref() == Some("now"),
                graph: match Contract::GRAPH.get_from(r) {
                    Some(g) => g.into(),
                    None => current_dir()?.join(commands::status::GRAPH_FILE),
                },
                root: match Contract::ROOT.get_from(r).filter(|s| !s.is_empty()) {
                    Some(root) => root.into(),
                    None => commands::status::default_root(Contract::HOME.get_from(r).as_deref())?,
                },
                worktrees: match Contract::AGENT_WORKTREES.get_from(r).filter(|s| !s.is_empty()) {
                    Some(d) => d.into(),
                    None => commands::assemble::default_worktrees(
                        Contract::HOME.get_from(r).as_deref(),
                    )?,
                },
                org: Contract::ORG.get_from(r).filter(|s| !s.is_empty()),
                tracking: Contract::TRACKED_REF.get_from(r).filter(|s| !s.is_empty()),
                upstream: Contract::UPSTREAM.get_from(r).filter(|s| !s.is_empty()),
                upstream_branch: Contract::UPSTREAM_BRANCH.get_from(r).filter(|s| !s.is_empty()),
                patches: Contract::PATCHES.get_from(r).filter(|s| !s.is_empty()),
                json: Contract::JSON.get_from(r).unwrap_or(false),
            })
        }
        Command::Status => {
            let root = match Contract::ROOT.get_from(r).filter(|s| !s.is_empty()) {
                Some(root) => root.into(),
                None => commands::status::default_root(Contract::HOME.get_from(r).as_deref())?,
            };
            let graph = match Contract::GRAPH.get_from(r) {
                Some(g) => g.into(),
                None => current_dir()?.join(commands::status::GRAPH_FILE),
            };
            Request::Status(Ask {
                graph,
                root,
                fetch: Contract::FETCH.get_from(r).unwrap_or(false),
                json: Contract::JSON.get_from(r).unwrap_or(false),
            })
        }
        Command::Check => Request::Check {
            graph: match Contract::GRAPH.get_from(r) {
                Some(g) => g.into(),
                None => current_dir()?.join(commands::status::GRAPH_FILE),
            },
        },
        Command::Assemble if Contract::DONE.get_from(r).is_some_and(|t| !t.is_empty()) => {
            Request::AssembleDone {
                tag: Contract::DONE.get_from(r).unwrap_or_default(),
                repo: match Contract::REPO.get_from(r).filter(|s| !s.is_empty()) {
                    Some(d) => d.into(),
                    None => current_dir()?,
                },
                force: Contract::FORCE.get_from(r).unwrap_or(false),
            }
        }
        Command::Follow if Contract::FOLLOW_EXTRA.get_from(r).is_some() => {
            return Err("usage: rhizoid follow --source DIR BASE VERSION".to_string())
        }
        Command::Follow => Request::Follow(commands::follow::Ask {
            source: Contract::SOURCE
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("usage: rhizoid follow --source DIR BASE VERSION")?
                .into(),
            base: Contract::FOLLOW_BASE
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("usage: rhizoid follow --source DIR BASE VERSION")?
                .into(),
            version: Contract::FOLLOW_VERSION
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("usage: rhizoid follow --source DIR BASE VERSION")?,
        }),
        Command::Bump => Request::Bump(commands::bump::Ask {
            graph: match Contract::GRAPH.get_from(r) {
                Some(g) => g.into(),
                None => current_dir()?.join(commands::status::GRAPH_FILE),
            },
            root: match Contract::ROOT.get_from(r).filter(|s| !s.is_empty()) {
                Some(root) => root.into(),
                None => commands::status::default_root(Contract::HOME.get_from(r).as_deref())?,
            },
            worktrees: match Contract::AGENT_WORKTREES.get_from(r).filter(|s| !s.is_empty()) {
                Some(d) => d.into(),
                None => {
                    commands::assemble::default_worktrees(Contract::HOME.get_from(r).as_deref())?
                }
            },
            now: Contract::ACTION.get_from(r).as_deref() == Some("now"),
            push: Contract::PUSH.get_from(r).unwrap_or(false),
            pr: Contract::PR.get_from(r).unwrap_or(false),
            land: Contract::LAND.get_from(r).unwrap_or(false),
            only: Contract::ONLY.get_from(r).filter(|s| !s.is_empty()),
            fetch: Contract::FETCH.get_from(r).unwrap_or(false),
            json: Contract::JSON.get_from(r).unwrap_or(false),
        }),
        Command::SendBack => Request::SendBack(commands::send_back::Ask {
            branch: Contract::BRANCH
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("send-back needs the branch to send")?,
            onto: Contract::ONTO
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("send-back needs --onto REF: the target tree it is for")?,
            out: Contract::OUT
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("send-back needs --out DIR: the folder the series goes in")?
                .into(),
            repo: match Contract::REPO.get_from(r).filter(|s| !s.is_empty()) {
                Some(d) => d.into(),
                None => current_dir()?,
            },
            worktrees: match Contract::AGENT_WORKTREES.get_from(r).filter(|s| !s.is_empty()) {
                Some(d) => d.into(),
                None => {
                    commands::assemble::default_worktrees(Contract::HOME.get_from(r).as_deref())?
                }
            },
            config: Contract::CONFIG.get_from(r).filter(|s| !s.is_empty()).map(Into::into),
            tested: Contract::TESTED.get_from(r).filter(|s| !s.is_empty()),
            fork: Contract::FORK.get_from(r).filter(|s| !s.is_empty()),
            json: Contract::JSON.get_from(r).unwrap_or(false),
        }),
        Command::Assemble => Request::Assemble(commands::assemble::Ask {
            list: Contract::LIST.get_from(r).ok_or("assemble needs the name of a list")?,
            lists: Contract::LISTS
                .get_from(r)
                .filter(|s| !s.is_empty())
                .ok_or("assemble needs the list file: --lists FILE, or RHIZOID_LISTS")?
                .into(),
            repo: match Contract::REPO.get_from(r).filter(|s| !s.is_empty()) {
                Some(d) => d.into(),
                None => current_dir()?,
            },
            worktrees: match Contract::AGENT_WORKTREES.get_from(r).filter(|s| !s.is_empty()) {
                Some(d) => d.into(),
                None => {
                    commands::assemble::default_worktrees(Contract::HOME.get_from(r).as_deref())?
                }
            },
            worktree: Contract::WORKTREE.get_from(r).filter(|s| !s.is_empty()).map(Into::into),
            fetch: Contract::FETCH.get_from(r).unwrap_or(false),
            reproduces: Contract::REPRODUCES.get_from(r).filter(|s| !s.is_empty()),
            json: Contract::JSON.get_from(r).unwrap_or(false),
        }),
        Command::Completions => {
            let shell = Contract::SHELL.get_from(r).and_then(|s| Shell::parse(&s));
            Request::Completions(
                shell.ok_or("completions needs a shell: bash, zsh, fish or nushell")?,
            )
        }
        Command::Man => Request::Man,
    })
}

fn main() -> ExitCode {
    let resolved = resolve();
    match request(&resolved) {
        Err(e) => {
            eprintln!("rhizoid: {e}");
            ExitCode::from(2)
        }
        Ok(Request::Help) => {
            print!("{}", cli::usage("rhizoid", ABOUT));
            ExitCode::SUCCESS
        }
        Ok(Request::Version) => {
            println!("rhizoid {VERSION}");
            ExitCode::SUCCESS
        }
        Ok(Request::Cutting(ask)) => match commands::cutting::run(&ask) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::Status(ask)) => match commands::status::run(&ask) {
            Ok(false) => ExitCode::SUCCESS,
            Ok(true) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::Check { graph }) => match commands::check::run(&graph, VERSION) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::AssembleDone { tag, repo, force }) => {
            match commands::assemble::done(&repo, &tag, force) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("rhizoid: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Ok(Request::Follow(ask)) => match commands::follow::run(&ask) {
            Ok(code) => ExitCode::from(code),
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::Bump(ask)) => match commands::bump::run(&ask) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::SendBack(ask)) => match commands::send_back::run(&ask) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::Assemble(ask)) => match commands::assemble::run(&ask) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rhizoid: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Request::Completions(sh)) => {
            emit(generate::completions(sh, &program(), &Contract::records()))
        }
        Ok(Request::Man) => emit(generate::man_page(&program(), &Contract::records())),
    }
}

fn done(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rhizoid: {e}");
            ExitCode::FAILURE
        }
    }
}

fn emit(r: Result<String, generate::GenerateError>) -> ExitCode {
    match r {
        Ok(s) => done(commands::status::write_report(&mut std::io::stdout().lock(), &s)),
        Err(e) => {
            eprintln!("rhizoid: {e}");
            ExitCode::FAILURE
        }
    }
}
