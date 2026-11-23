//! `rhizoid` - fork, own, and sync git dependencies instead of relying on a
//! package manager. The invocation surface is declared once, in `cli.rs`
//! (argenv); this file reads it and runs the command.
//!
//! Exit status: 0 done; 1 `status` found drift or a command failed; 2 the
//! invocation is wrong or `status` could not read the world.

mod cli;
mod commands;
mod git_cli;
mod git_repos;
mod manifest_io;

use argenv_resolve::generate::{self, Program, Shell};
use argenv_resolve::{lint, InputResolveExt, LintPolicy, Resolution, Severity};
use cli::{Command, Contract};
use commands::status::Ask;
use commands::unbuilt::Verb;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAN_DATE: &str = "2026-11-23";
const ABOUT: &str = "Fork, own and sync git dependencies, report every edge of a repository graph (rhizoid status), merge a named list of branches onto a base into a tag (rhizoid assemble), and draft the next series version of a branch for its upstream (rhizoid send-back)";

const COMMANDS: &[generate::Command<'static>] = &[
    generate::Command { name: "init", summary: "Write a fresh rhizoid.toml" },
    generate::Command { name: "add", summary: "Fork a new module into the configured org" },
    generate::Command { name: "import", summary: "Adopt an existing fork" },
    generate::Command { name: "update", summary: "Pull upstream into a module's tracking branch" },
    generate::Command { name: "refresh", summary: "Reconcile the manifest with what is observed" },
    generate::Command { name: "remove", summary: "Drop a module from the manifest" },
    generate::Command {
        name: "status",
        summary: "Every edge of a repository graph: current, behind or moved by hand, and how",
    },
    generate::Command {
        name: "assemble",
        summary: "Merge a named list of branches onto its base in a fresh worktree, and tag it int/DEVICE/LIST/BASE/N",
    },
    generate::Command {
        name: "send-back",
        summary: "Draft the next series version of a branch for its upstream: rebased, cut, range-diffed, checked, tagged as a draft",
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
    Init { dir: PathBuf, org: Option<String>, force: bool },
    Remove { dir: PathBuf, module: String },
    Unbuilt { verb: Verb, target: Option<String> },
    Status(Ask),
    Assemble(commands::assemble::Ask),
    AssembleDone { tag: String, repo: PathBuf, force: bool },
    SendBack(commands::send_back::Ask),
    Completions(Shell),
    Man,
}

fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("cannot read the current folder: {e}"))
}

/// The target a command needs, or the usage error that says which.
fn required(target: Option<String>, what: &str) -> Result<String, String> {
    target.ok_or_else(|| format!("missing <{what}>"))
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
        Command::Init => Request::Init {
            dir: current_dir()?,
            org: Contract::ORG.get_from(r).filter(|s| !s.is_empty()),
            force: Contract::FORCE.get_from(r).unwrap_or(false),
        },
        Command::Remove => {
            Request::Remove { dir: current_dir()?, module: required(target, "module")? }
        }
        Command::Add => {
            Request::Unbuilt { verb: Verb::Add, target: Some(required(target, "source")?) }
        }
        Command::Import => {
            Request::Unbuilt { verb: Verb::Import, target: Some(required(target, "fork")?) }
        }
        Command::Update => Request::Unbuilt { verb: Verb::Update, target },
        Command::Refresh => Request::Unbuilt { verb: Verb::Refresh, target },
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
        Ok(Request::Init { dir, org, force }) => done(commands::init::run(&dir, org, force)),
        Ok(Request::Remove { dir, module }) => done(commands::remove::run(&dir, &module)),
        Ok(Request::Unbuilt { verb, target }) => {
            done(commands::unbuilt::run(verb, target.as_deref()))
        }
        Ok(Request::Status(ask)) => match commands::status::run(&ask) {
            Ok(false) => ExitCode::SUCCESS,
            Ok(true) => ExitCode::FAILURE,
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
