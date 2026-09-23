//! `rhizoid` - fork, own, and sync git dependencies instead of relying on a
//! package manager.
//!
//! ```text
//! rhizoid init [--org ORG] [--force]
//! rhizoid add <owner/repo> [--org ORG] [--as NAME] [--tracked-ref REF]
//! rhizoid import <owner/repo> [--as NAME] [--tracked-ref REF]
//! rhizoid update [module]
//! rhizoid status [--json]
//! rhizoid refresh [module]
//! rhizoid remove <module>
//! ```
//!
//! Subcommand dispatch is a plain match on the first argument, matching
//! argenv-cli's own reference pattern - see `commands` module docs for why
//! that, and each command's leading positional, are hand-parsed rather than
//! declared through argenv.

mod commands;
mod manifest_io;

use argenv::{lint, Invocation, ProcessEnv};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::env::current_dir().expect("current directory is readable");

    match args.first().map(String::as_str) {
        Some("init") => run_flags_only(
            "init",
            &args[1..],
            commands::init::Model::records(),
            commands::init::Model::problems(),
            |resolved| commands::init::run(&dir, resolved),
        ),
        Some("add") => run_with_required_positional(
            "add",
            &args[1..],
            "source",
            commands::add::Model::records(),
            commands::add::Model::problems(),
            commands::add::run,
        ),
        Some("import") => run_with_required_positional(
            "import",
            &args[1..],
            "fork",
            commands::import::Model::records(),
            commands::import::Model::problems(),
            commands::import::run,
        ),
        Some("update") => run_with_optional_positional(
            "update",
            &args[1..],
            commands::update::Model::records(),
            commands::update::Model::problems(),
            commands::update::run,
        ),
        Some("refresh") => run_with_optional_positional(
            "refresh",
            &args[1..],
            commands::refresh::Model::records(),
            commands::refresh::Model::problems(),
            commands::refresh::run,
        ),
        Some("remove") => run_with_required_positional(
            "remove",
            &args[1..],
            "module",
            commands::remove::Model::records(),
            commands::remove::Model::problems(),
            |name, _resolved| commands::remove::run(&dir, name),
        ),
        Some("status") => {
            check_model("status", commands::status::Model::problems());
            let manifest = match manifest_io::load(&dir) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("rhizoid: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let records = commands::status::Model::records();
            let resolved = Invocation { args: &args[1..], env: &ProcessEnv }.resolve(&records);
            report_findings(&records, &args[1..]);
            commands::status::run(&manifest, &resolved);
            ExitCode::SUCCESS
        }
        Some("--version") | Some("-V") => {
            println!("rhizoid {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("rhizoid: unknown command `{other}`");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "rhizoid {}\n\
         \n\
         USAGE\n    \
             rhizoid init [--org ORG] [--force]\n    \
             rhizoid add <owner/repo> [--org ORG] [--as NAME] [--tracked-ref REF]\n    \
             rhizoid import <owner/repo> [--as NAME] [--tracked-ref REF]\n    \
             rhizoid update [module]\n    \
             rhizoid status [--json]\n    \
             rhizoid refresh [module]\n    \
             rhizoid remove <module>\n    \
             rhizoid --version | --help\n",
        env!("CARGO_PKG_VERSION")
    );
}

/// Check a command's own `Model::problems()` before resolving anything -
/// catches a bad `Input` declaration (argenv's own contract self-check, see
/// its `Input::check`) at the moment it would matter, not just in a test
/// suite. Should never fire in practice; if it does, it is a bug in this
/// binary, not in whatever the person typed.
fn check_model(command: &str, problems: Vec<String>) {
    if !problems.is_empty() {
        eprintln!("rhizoid: internal error - `{command}`'s own declaration is invalid:");
        for p in problems {
            eprintln!("  {p}");
        }
        std::process::exit(70); // EX_SOFTWARE
    }
}

/// Print any lint findings (unknown flags, out-of-domain values) to stderr.
/// A finding is a warning about the invocation, not by itself a reason to
/// abort - argenv's own contract for `lint` distinguishes `Severity::Error`
/// findings, which callers may want to treat as fatal in a later pass.
fn report_findings(records: &[argenv::Record], args: &[String]) {
    let invocation = Invocation { args, env: &ProcessEnv };
    for finding in lint(records, &invocation) {
        eprintln!("{:?}: {finding}", finding.severity());
    }
}

fn run_flags_only(
    command: &str,
    args: &[String],
    records: Vec<argenv::Record>,
    problems: Vec<String>,
    body: impl FnOnce(&argenv::Resolution) -> Result<(), String>,
) -> ExitCode {
    check_model(command, problems);
    report_findings(&records, args);
    let resolved = Invocation { args, env: &ProcessEnv }.resolve(&records);
    match body(&resolved) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rhizoid: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_with_required_positional(
    command: &str,
    args: &[String],
    what: &str,
    records: Vec<argenv::Record>,
    problems: Vec<String>,
    body: impl FnOnce(&str, &argenv::Resolution) -> Result<(), String>,
) -> ExitCode {
    check_model(command, problems);
    let Some(positional) = args.first() else {
        eprintln!("rhizoid: missing <{what}>");
        return ExitCode::from(2);
    };
    if positional.starts_with('-') {
        eprintln!("rhizoid: expected <{what}> before any flags, found `{positional}`");
        return ExitCode::from(2);
    }
    let rest = &args[1..];
    report_findings(&records, rest);
    let resolved = Invocation { args: rest, env: &ProcessEnv }.resolve(&records);
    match body(positional, &resolved) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rhizoid: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_with_optional_positional(
    command: &str,
    args: &[String],
    records: Vec<argenv::Record>,
    problems: Vec<String>,
    body: impl FnOnce(Option<&str>, &argenv::Resolution) -> Result<(), String>,
) -> ExitCode {
    check_model(command, problems);
    let (positional, rest): (Option<&str>, &[String]) = match args.first() {
        Some(first) if !first.starts_with('-') => (Some(first.as_str()), &args[1..]),
        _ => (None, args),
    };
    report_findings(&records, rest);
    let resolved = Invocation { args: rest, env: &ProcessEnv }.resolve(&records);
    match body(positional, &resolved) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rhizoid: {e}");
            ExitCode::FAILURE
        }
    }
}
