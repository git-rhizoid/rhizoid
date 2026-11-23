//! rhizoid's invocation surface, declared once with argenv: its commands, their arguments and the environment
//! variables it reads. The parser, `--help`, the manual page and the completions all come from this.
//!
//! ```text
//!   rhizoid add SOURCE [now] [--org ORG] [--tracked-ref REF] [--patches BRANCH|integration] [--graph FILE] [--root DIR]
//!   rhizoid import FORK [now] [--upstream REMOTE] [--upstream-branch B] [--tracked-ref REF] [--patches ...]
//!   rhizoid update MODULE [now]       rhizoid refresh       rhizoid remove MODULE [now]     (cuttings, in the graph)
//!   rhizoid status [--graph FILE] [--root DIR] [--fetch] [--json]   every edge of a repository graph, read-only
//!   rhizoid check [--graph FILE]      whether this version can read the graph: parsed and checked, no clones, no network
//!   rhizoid assemble LIST [--lists FILE] [--repo DIR] [--worktree DIR] [--reproduces COMMIT] [--fetch] [--json]
//!   rhizoid assemble --done TAG [--repo DIR] [--force]  end a tag's worktree
//!   rhizoid send-back BRANCH --onto REF --out DIR [--config FILE] [--tested SHA] [--fork ID] [--repo DIR] [--json]
//!   rhizoid bump [now] [--graph FILE] [--root DIR] [--only REPO] [--fetch] [--push] [--pr] [--land] [--json]   every pin in one place; now bumps
//!   rhizoid follow --source DIR BASE VERSION    lay a release of a source into a base, as idealkb's follow.sh does
//!   rhizoid completions SHELL | man | --version | --help
//! ```

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Add,
    Import,
    Update,
    Refresh,
    Remove,
    Status,
    Check,
    Assemble,
    #[serde(rename = "send-back")]
    SendBack,
    Bump,
    Follow,
    Completions,
    Man,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &[
        "add",
        "import",
        "update",
        "refresh",
        "remove",
        "status",
        "check",
        "assemble",
        "send-back",
        "bump",
        "follow",
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
            "add" => Command::Add,
            "import" => Command::Import,
            "update" => Command::Update,
            "refresh" => Command::Refresh,
            "remove" => Command::Remove,
            "status" => Command::Status,
            "check" => Command::Check,
            "assemble" => Command::Assemble,
            "send-back" => Command::SendBack,
            "bump" => Command::Bump,
            "follow" => Command::Follow,
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
        subcommands: &[Command::Add],
        env: Some(Env::new("RHIZOID_ORG")),
        arg: Some(Arg { value_name: "ORG", ..Arg::long("org") }),
        summary: "The GitHub org or user a new cutting is forked into (default: the graph's fork_org).",
        ..Input::EMPTY
    };

    FORCE: bool, Command = Input {
        key: "force",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Assemble],
        arg: Some(Arg::long("force")),
        summary: "assemble --done: remove the worktree even with changes in it.",
        ..Input::EMPTY
    };

    TRACKED_REF: String, Command = Input {
        key: "tracked_ref",
        ty: Type::String,
        subcommands: &[Command::Add, Command::Import],
        arg: Some(Arg { value_name: "REF", ..Arg::long("tracked-ref") }),
        summary: "The cutting's tracking branch (default: the graph's, or the fork's default branch).",
        ..Input::EMPTY
    };

    GRAPH: String, Command = Input {
        key: "graph",
        ty: Type::String,
        subcommands: &[Command::Status, Command::Check, Command::Bump, Command::Add, Command::Import, Command::Update, Command::Refresh, Command::Remove],
        env: Some(Env::new("RHIZOID_GRAPH")),
        arg: Some(Arg { value_name: "FILE", ..Arg::long("graph") }),
        summary: "The graph file: the edges to read (rhizoid.graph.toml here when none is given).",
        ..Input::EMPTY
    };

    ROOT: String, Command = Input {
        key: "root",
        ty: Type::String,
        subcommands: &[Command::Status, Command::Bump, Command::Add, Command::Import, Command::Update, Command::Refresh, Command::Remove],
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
        subcommands: &[Command::Status, Command::Assemble, Command::Bump],
        arg: Some(Arg::long("fetch")),
        summary: "Fetch first: status, each clone's tags and branches; assemble, every remote of the clone, pruning (otherwise the clones are read as they are).",
        ..Input::EMPTY
    };

    JSON: bool, Command = Input {
        key: "json",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Status, Command::Assemble, Command::SendBack, Command::Bump, Command::Add, Command::Import, Command::Update, Command::Refresh, Command::Remove],
        arg: Some(Arg::long("json")),
        summary: "Print the report as JSON (status: the findings; assemble: the recipe and the outcome; send-back: the series).",
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
        summary: "The list file: its named lists, its tag template, the branches a list may name (no default: every user keeps its own).",
        ..Input::EMPTY
    };

    REPO: String, Command = Input {
        key: "repo",
        ty: Type::String,
        subcommands: &[Command::Assemble, Command::SendBack],
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

    DONE: String, Command = Input {
        key: "done",
        ty: Type::String,
        subcommands: &[Command::Assemble],
        arg: Some(Arg { value_name: "TAG", ..Arg::long("done") }),
        summary: "End a tag's worktree instead of assembling: unlock and remove it (refused when it has changes, unless --force).",
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

    BRANCH: String, Command = Input {
        key: "branch",
        ty: Type::String,
        subcommands: &[Command::SendBack],
        arg: Some(Arg { value_name: "BRANCH", ..Arg::positional(0) }),
        summary: "The branch to draft the next series version of.",
        ..Input::EMPTY
    };

    ONTO: String, Command = Input {
        key: "onto",
        ty: Type::String,
        subcommands: &[Command::SendBack],
        arg: Some(Arg { value_name: "REF", ..Arg::long("onto") }),
        summary: "The target to rebase onto: the upstream tree's ref the series is for (a maintainer's for-next).",
        ..Input::EMPTY
    };

    OUT: String, Command = Input {
        key: "out",
        ty: Type::String,
        subcommands: &[Command::SendBack],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("out") }),
        summary: "The folder the draft goes in: new, empty, or this same draft's from an earlier run (printed as file=).",
        ..Input::EMPTY
    };

    CONFIG: String, Command = Input {
        key: "config",
        ty: Type::String,
        subcommands: &[Command::SendBack],
        env: Some(Env::new("RHIZOID_CONFIG")),
        arg: Some(Arg { value_name: "FILE", ..Arg::long("config") }),
        summary: "rhizoid's config: its [send] section names the send branch and version tag, and the check and recipient commands (none: no checks).",
        ..Input::EMPTY
    };

    TESTED: String, Command = Input {
        key: "tested",
        ty: Type::String,
        subcommands: &[Command::SendBack],
        arg: Some(Arg { value_name: "SHA", ..Arg::long("tested") }),
        summary: "The commit the series was tested on: a commit's sha, never a name that moves.",
        ..Input::EMPTY
    };

    FORK: String, Command = Input {
        key: "fork",
        ty: Type::String,
        subcommands: &[Command::SendBack],
        arg: Some(Arg { value_name: "ID", ..Arg::long("fork") }),
        summary: "The fork's id among the caller's upstreams: printed as carried-in=ID/BRANCH.",
        ..Input::EMPTY
    };

    ACTION: String, Command = Input {
        key: "action",
        ty: Type::Enum,
        allowed: &["now"],
        subcommands: &[Command::Bump],
        arg: Some(Arg { value_name: "now", ..Arg::positional(0) }),
        summary: "Bump now: without it, bump only lists every pin and what it would do.",
        ..Input::EMPTY
    };

    ONLY: String, Command = Input {
        key: "only",
        ty: Type::String,
        subcommands: &[Command::Bump],
        arg: Some(Arg { value_name: "REPO", ..Arg::long("only") }),
        summary: "Only this follower's pins (owner/repo).",
        ..Input::EMPTY
    };

    PUSH: bool, Command = Input {
        key: "push",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Bump],
        arg: Some(Arg::long("push")),
        summary: "Push each bump's branch to the follower's origin.",
        ..Input::EMPTY
    };

    PR: bool, Command = Input {
        key: "pr",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Bump],
        arg: Some(Arg::long("pr")),
        summary: "Push each bump's branch and open its pull request (gh pr create --fill).",
        ..Input::EMPTY
    };

    LAND: bool, Command = Input {
        key: "land",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Bump],
        arg: Some(Arg::long("land")),
        summary: "Push, open the pull request and land each bump through the follower's checks: the graph's [bump] land argv.",
        ..Input::EMPTY
    };

    SOURCE: String, Command = Input {
        key: "source",
        ty: Type::String,
        subcommands: &[Command::Follow],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("source") }),
        summary: "A clone of the source that holds the tag vVERSION.",
        ..Input::EMPTY
    };

    FOLLOW_BASE: String, Command = Input {
        key: "follow_base",
        ty: Type::String,
        subcommands: &[Command::Follow],
        arg: Some(Arg { value_name: "BASE", ..Arg::positional(0) }),
        summary: "The base to lay the release into.",
        ..Input::EMPTY
    };

    FOLLOW_VERSION: String, Command = Input {
        key: "follow_version",
        ty: Type::String,
        subcommands: &[Command::Follow],
        arg: Some(Arg { value_name: "VERSION", ..Arg::positional(1) }),
        summary: "The release to lay (its tag is vVERSION).",
        ..Input::EMPTY
    };

    FOLLOW_EXTRA: String, Command = Input {
        key: "follow_extra",
        ty: Type::String,
        subcommands: &[Command::Follow],
        arg: Some(Arg { value_name: "NOTHING", ..Arg::positional(2) }),
        summary: "Nothing goes after VERSION: anything there is refused.",
        ..Input::EMPTY
    };

    NOW: String, Command = Input {
        key: "now",
        ty: Type::Enum,
        allowed: &["now"],
        subcommands: &[Command::Add, Command::Import, Command::Update, Command::Remove],
        arg: Some(Arg { value_name: "now", ..Arg::positional(1) }),
        summary: "Do it now: without it a cutting verb prints its plan and changes nothing.",
        ..Input::EMPTY
    };

    PATCHES: String, Command = Input {
        key: "patches",
        ty: Type::String,
        subcommands: &[Command::Add, Command::Import],
        arg: Some(Arg { value_name: "BRANCH|integration", ..Arg::long("patches") }),
        summary: "Where the cutting's changes live: one branch (rhizoid/patch by default) or integration (the topics its [[integration]] edge assembles).",
        ..Input::EMPTY
    };

    UPSTREAM: String, Command = Input {
        key: "upstream",
        ty: Type::String,
        subcommands: &[Command::Import],
        arg: Some(Arg { value_name: "REMOTE", ..Arg::long("upstream") }),
        summary: "The cutting's upstream (owner/repo or a URL), when GitHub's parent is not it.",
        ..Input::EMPTY
    };

    UPSTREAM_BRANCH: String, Command = Input {
        key: "upstream_branch",
        ty: Type::String,
        subcommands: &[Command::Add, Command::Import],
        arg: Some(Arg { value_name: "BRANCH", ..Arg::long("upstream-branch") }),
        summary: "The upstream's branch the tracking branch follows (default: the tracking branch's name).",
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
        let r = resolve(&["add", "a/b", "--org", "from-arg"], &[("RHIZOID_ORG", "from-env")]);
        assert_eq!(Contract::ORG.get_from(&r).as_deref(), Some("from-arg"));
        let r = resolve(&["add", "a/b"], &[("RHIZOID_ORG", "from-env")]);
        assert_eq!(Contract::ORG.get_from(&r).as_deref(), Some("from-env"));
    }

    #[test]
    fn the_target_comes_after_the_command_and_before_or_among_its_flags() {
        let r = resolve(&["add", "octocat/hello", "--tracked-ref", "main"], &[]);
        assert_eq!(Contract::TARGET.get_from(&r).as_deref(), Some("octocat/hello"));
        assert_eq!(Contract::TRACKED_REF.get_from(&r).as_deref(), Some("main"));
        let r = resolve(&["add", "--tracked-ref", "main", "octocat/hello", "now"], &[]);
        assert_eq!(Contract::NOW.get_from(&r).as_deref(), Some("now"));
        assert_eq!(Contract::TARGET.get_from(&r).as_deref(), Some("octocat/hello"));
    }

    #[test]
    fn a_flag_given_to_the_wrong_command_is_a_finding() {
        let r = resolve(&["follow", "--json"], &[]);
        let findings = lint(&Contract::records(), &r);
        assert!(!findings.is_empty(), "--json belongs to status");
    }
}
