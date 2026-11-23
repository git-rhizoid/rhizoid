//! rhizoid's invocation surface, declared once with argenv: its commands, their arguments and the environment
//! variables it reads. The parser, `--help`, the manual page and the completions all come from this.
//!
//! ```text
//!   rhizoid init [--org ORG] [--force]                 write a fresh rhizoid.toml
//!   rhizoid add SOURCE [--org ORG] [--as NAME] [--tracked-ref REF]
//!   rhizoid import FORK [--as NAME] [--tracked-ref REF]
//!   rhizoid update [MODULE]
//!   rhizoid refresh [MODULE]
//!   rhizoid remove MODULE
//!   rhizoid status [--graph FILE] [--root DIR] [--fetch] [--json]   every edge of a repository graph (kernel clones too), read-only
//!   rhizoid assemble LIST [--lists FILE] [--repo DIR] [--worktree DIR] [--reproduces COMMIT] [--fetch] [--json]
//!   rhizoid completions SHELL | man | --version | --help
//! ```

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Init,
    Add,
    Import,
    Update,
    Refresh,
    Remove,
    Status,
    Assemble,
    Completions,
    Man,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &[
        "init",
        "add",
        "import",
        "update",
        "refresh",
        "remove",
        "status",
        "assemble",
        "completions",
        "man",
    ];
    /// The commands that take one module or repository as their first argument.
    pub const TARGETED: &'static [Command] =
        &[Command::Add, Command::Import, Command::Update, Command::Refresh, Command::Remove];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "init" => Command::Init,
            "add" => Command::Add,
            "import" => Command::Import,
            "update" => Command::Update,
            "refresh" => Command::Refresh,
            "remove" => Command::Remove,
            "status" => Command::Status,
            "assemble" => Command::Assemble,
            "completions" => Command::Completions,
            "man" => Command::Man,
            _ => return None,
        })
    }
}

contract! {
    COMMAND: Command = Input {
        key: "command",
        ty: Type::Enum,
        allowed: Command::TOKENS,
        arg: Some(Arg::positional(0)),
        summary: "What to do.",
        ..Input::EMPTY
    };

    TARGET: String, Command = Input {
        key: "target",
        ty: Type::String,
        subcommands: Command::TARGETED,
        arg: Some(Arg { value_name: "NAME", ..Arg::positional(0) }),
        summary: "The upstream (add), the fork (import) or the module (update, refresh, remove) the command is about.",
        ..Input::EMPTY
    };

    SHELL: String, Command = Input {
        key: "shell",
        ty: Type::Enum,
        allowed: &["bash", "zsh", "fish", "nushell"],
        subcommands: &[Command::Completions],
        arg: Some(Arg::positional(0)),
        summary: "The shell to write completions for.",
        ..Input::EMPTY
    };

    ORG: String, Command = Input {
        key: "org",
        ty: Type::String,
        subcommands: &[Command::Init, Command::Add],
        env: Some(Env::new("RHIZOID_ORG")),
        arg: Some(Arg { value_name: "ORG", ..Arg::long("org") }),
        summary: "The GitHub org or user new forks are created under (init: by default; add: for this module only).",
        ..Input::EMPTY
    };

    FORCE: bool, Command = Input {
        key: "force",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Init],
        arg: Some(Arg::long("force")),
        summary: "Overwrite an existing rhizoid.toml.",
        ..Input::EMPTY
    };

    AS_NAME: String, Command = Input {
        key: "as_name",
        ty: Type::String,
        subcommands: &[Command::Add, Command::Import],
        arg: Some(Arg { value_name: "NAME", ..Arg::long("as") }),
        summary: "The module's name in the manifest (the repository's own name when none is given).",
        ..Input::EMPTY
    };

    TRACKED_REF: String, Command = Input {
        key: "tracked_ref",
        ty: Type::String,
        subcommands: &[Command::Add, Command::Import],
        arg: Some(Arg { value_name: "REF", ..Arg::long("tracked-ref") }),
        summary: "The branch or tag on the fork that tracks upstream.",
        ..Input::EMPTY
    };

    GRAPH: String, Command = Input {
        key: "graph",
        ty: Type::String,
        subcommands: &[Command::Status],
        env: Some(Env::new("RHIZOID_GRAPH")),
        arg: Some(Arg { value_name: "FILE", ..Arg::long("graph") }),
        summary: "The graph file: the edges to read (rhizoid.graph.toml here when none is given).",
        ..Input::EMPTY
    };

    ROOT: String, Command = Input {
        key: "root",
        ty: Type::String,
        subcommands: &[Command::Status],
        env: Some(Env::new("RHIZOID_ROOT")),
        arg: Some(Arg { value_name: "DIR", ..Arg::long("root") }),
        summary: "The folder the clones are under (~/Projects when none is given).",
        ..Input::EMPTY
    };

    HOME: String = Input {
        key: "home",
        ty: Type::String,
        env: Some(Env::new("HOME")),
        summary: "The user's home: where ~/Projects is.",
        ..Input::EMPTY
    };

    FETCH: bool, Command = Input {
        key: "fetch",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Status, Command::Assemble],
        arg: Some(Arg::long("fetch")),
        summary: "Fetch first: status, each clone's tags and branches; assemble, every remote of the clone, pruning (otherwise the clones are read as they are).",
        ..Input::EMPTY
    };

    JSON: bool, Command = Input {
        key: "json",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Status, Command::Assemble],
        arg: Some(Arg::long("json")),
        summary: "Print the report as JSON (status: the findings; assemble: the recipe and the outcome).",
        ..Input::EMPTY
    };

    LIST: String, Command = Input {
        key: "list",
        ty: Type::String,
        subcommands: &[Command::Assemble],
        arg: Some(Arg { value_name: "LIST", ..Arg::positional(0) }),
        summary: "The named list to assemble ([lists.LIST] of the list file).",
        ..Input::EMPTY
    };

    LISTS: String, Command = Input {
        key: "lists",
        ty: Type::String,
        subcommands: &[Command::Assemble],
        env: Some(Env::new("RHIZOID_LISTS")),
        arg: Some(Arg { value_name: "FILE", ..Arg::long("lists") }),
        summary: "The lab's list file (kernel/topics.toml here when none is given).",
        ..Input::EMPTY
    };

    REPO: String, Command = Input {
        key: "repo",
        ty: Type::String,
        subcommands: &[Command::Assemble],
        env: Some(Env::new("RHIZOID_REPO")),
        arg: Some(Arg { value_name: "DIR", ..Arg::long("repo") }),
        summary: "The clone the branches are in (the current folder when none is given).",
        ..Input::EMPTY
    };

    WORKTREE: String, Command = Input {
        key: "worktree",
        ty: Type::String,
        subcommands: &[Command::Assemble],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("worktree") }),
        summary: "The fresh worktree to assemble in; it must be inside AGENT_WORKTREES/REPO/rhizoid (DEVICE-LIST-BASE-N there when none is given).",
        ..Input::EMPTY
    };

    REPRODUCES: String, Command = Input {
        key: "reproduces",
        ty: Type::String,
        subcommands: &[Command::Assemble],
        arg: Some(Arg { value_name: "COMMIT", ..Arg::long("reproduces") }),
        summary: "A commit whose tree the tag's tree must equal; the paths that differ are listed.",
        ..Input::EMPTY
    };

    AGENT_WORKTREES: String = Input {
        key: "agent_worktrees",
        ty: Type::String,
        env: Some(Env::new("AGENT_WORKTREES")),
        summary: "The agent worktree home (~/.local/share/agent-worktrees when unset); assemble's worktrees go in REPO/rhizoid under it.",
        ..Input::EMPTY
    };

    HELP: bool = Input {
        key: "help",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg { short: Some('h'), ..Arg::long("help") }),
        summary: "Print this usage and stop.",
        ..Input::EMPTY
    };

    VERSION: bool = Input {
        key: "version",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg { short: Some('V'), ..Arg::long("version") }),
        summary: "Print the version and stop.",
        ..Input::EMPTY
    };
}

/// The usage `--help` prints: the program's line, then every declared argument and environment
/// variable with its summary, all read from the declaration above.
pub fn usage(name: &str, about: &str) -> String {
    let mut out = format!("Usage: {name} COMMAND [OPTIONS] [NAME]\n\n{about}\n");
    let records = Contract::records();
    let mut args = String::new();
    let mut envs = String::new();
    for r in &records {
        let summary = r.summary.clone().unwrap_or_default();
        let sig = r.usage();
        if !sig.is_empty() {
            args.push_str(&format!("  {sig:<22} {summary}\n"));
        }
        for e in r.env_names() {
            envs.push_str(&format!("  {e:<22} {summary}\n"));
        }
    }
    if !args.is_empty() {
        out.push_str(&format!("\nArguments:\n{args}"));
    }
    if !envs.is_empty() {
        out.push_str(&format!("\nEnvironment:\n{envs}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn resolve(args: &[&str], env: &[(&str, &str)]) -> Resolution {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let env: BTreeMap<String, String> =
            env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Invocation { args: &args, env: &env }.resolve(&Contract::records())
    }

    #[test]
    fn the_declaration_holds() {
        assert!(Contract::problems().is_empty(), "{:#?}", Contract::problems());
    }

    #[test]
    fn every_command_token_is_read_back_as_its_command() {
        for t in Command::TOKENS {
            assert!(Command::from_raw(t).is_some(), "{t}");
        }
    }

    #[test]
    fn status_reads_its_graph_and_root_from_flags_or_the_environment() {
        let r = resolve(&["status", "--graph", "g.toml", "--json"], &[("RHIZOID_ROOT", "/p")]);
        assert_eq!(Contract::COMMAND.get_from(&r), Some(Command::Status));
        assert_eq!(Contract::GRAPH.get_from(&r).as_deref(), Some("g.toml"));
        assert_eq!(Contract::ROOT.get_from(&r).as_deref(), Some("/p"));
        assert_eq!(Contract::JSON.get_from(&r), Some(true));
    }

    #[test]
    fn a_flag_beats_the_environment() {
        let r = resolve(&["init", "--org", "from-arg"], &[("RHIZOID_ORG", "from-env")]);
        assert_eq!(Contract::ORG.get_from(&r).as_deref(), Some("from-arg"));
        let r = resolve(&["init"], &[("RHIZOID_ORG", "from-env")]);
        assert_eq!(Contract::ORG.get_from(&r).as_deref(), Some("from-env"));
    }

    #[test]
    fn the_target_comes_after_the_command_and_before_or_among_its_flags() {
        let r = resolve(&["add", "octocat/hello", "--as", "renamed"], &[]);
        assert_eq!(Contract::TARGET.get_from(&r).as_deref(), Some("octocat/hello"));
        assert_eq!(Contract::AS_NAME.get_from(&r).as_deref(), Some("renamed"));
        let r = resolve(&["add", "--as", "renamed", "octocat/hello"], &[]);
        assert_eq!(Contract::TARGET.get_from(&r).as_deref(), Some("octocat/hello"));
    }

    #[test]
    fn a_flag_given_to_the_wrong_command_is_a_finding() {
        let r = resolve(&["init", "--json"], &[]);
        let findings = lint(&Contract::records(), &r);
        assert!(!findings.is_empty(), "--json belongs to status");
    }
}
