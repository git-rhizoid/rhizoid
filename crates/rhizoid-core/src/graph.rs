//! The graph file: the edges of one repository graph, read once at the edge into types that hold the rules.
//!
//! A follower's own pin files say which release it follows; they do not say which repositories follow
//! which source, nor what a source's tags look like. That is what `rhizoid.graph.toml` says, and nothing
//! more: a name misspelled, a pin that reads two ways, a repository that is not `owner/repo` is refused
//! here, not found out later in a check.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

/// A repository as GitHub names it: `owner/repo`, nothing else.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, serde::Serialize)]
#[serde(try_from = "String")]
pub struct RepoName(String);

impl RepoName {
    /// The name as `owner/repo`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RepoName {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let ok = match s.split_once('/') {
            Some((o, r)) => [o, r].iter().all(|p| {
                !p.is_empty()
                    && !p.starts_with('-')
                    && !p.chars().all(|c| c == '.')
                    && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            }),
            None => false,
        };
        if ok {
            Ok(RepoName(s))
        } else {
            Err(format!("`{s}` is not a repository name (owner/repo)"))
        }
    }
}

impl fmt::Display for RepoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where a git remote is: a GitHub `owner/repo`, or any URL or path git can ask.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub enum Remote {
    /// A repository on GitHub.
    GitHub(RepoName),
    /// Any other place `git ls-remote` accepts.
    Url(String),
}

impl Remote {
    /// What to give `git ls-remote`.
    pub fn url(&self) -> String {
        match self {
            Remote::GitHub(n) => format!("https://github.com/{n}"),
            Remote::Url(u) => u.clone(),
        }
    }
}

impl TryFrom<String> for Remote {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        if s.starts_with('-') {
            Err(format!("`{s}` starts with a dash: git would read it as an option"))
        } else if s.contains("://") || s.starts_with('/') || s.starts_with('.') {
            Ok(Remote::Url(s))
        } else {
            RepoName::try_from(s).map(Remote::GitHub)
        }
    }
}

impl fmt::Display for Remote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Remote::GitHub(n) => write!(f, "{n}"),
            Remote::Url(u) => write!(f, "{u}"),
        }
    }
}

fn default_rev() -> Rev {
    Rev("origin/main".to_string())
}
fn default_tag_prefix() -> String {
    "v".to_string()
}
fn default_part_file() -> String {
    "tables.toml".to_string()
}
fn default_items() -> String {
    "data/upstream_items.json".to_string()
}
fn default_own_key() -> String {
    "own".to_string()
}
fn default_why_key() -> String {
    "why".to_string()
}

/// The whole graph: the revision every clone is read at, where clones differ from their names, and the edges by kind.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    /// The revision each clone is read at (`origin/main`).
    #[serde(default = "default_rev")]
    pub rev: Rev,
    /// Where a clone lives under the root when it is not `owner/repo`.
    #[serde(default)]
    pub clones: BTreeMap<RepoName, String>,
    /// Sources that give files to followers by a pin file.
    #[serde(default, rename = "slice")]
    pub slices: Vec<Slice>,
    /// Device layers: a source pinned by a release, whose contract has one reader.
    #[serde(default, rename = "layer")]
    pub layers: Vec<Layer>,
    /// Tools pinned at a release of their source.
    #[serde(default, rename = "tool")]
    pub tools: Vec<Tool>,
    /// Forks whose tracking branch should be their upstream's head.
    #[serde(default, rename = "fork")]
    pub forks: Vec<Fork>,
    /// The GitHub org or user a new cutting is forked into when `add` names none.
    #[serde(default)]
    pub fork_org: Option<String>,
    /// Repositories whose branches are assembled into tags: their names, topics, and what assemble made.
    #[serde(default, rename = "integration")]
    pub integrations: Vec<crate::integration::Integration>,
}

impl Graph {
    /// Read a graph file's text; a misspelled key or a malformed name is an error naming it.
    pub fn parse(text: &str) -> Result<Graph, String> {
        let graph: Graph = toml::from_str(text).map_err(|e| e.to_string())?;
        for (repo, path) in &graph.clones {
            let plain = !path.is_empty()
                && !path.starts_with('/')
                && !path.starts_with('-')
                && !path.split('/').any(|p| p == "..");
            if !plain {
                return Err(format!(
                    "the clone of {repo} is at `{path}`: a folder under the root, no `..`"
                ));
            }
        }
        for f in &graph.forks {
            if let Some(a) = f.added.as_deref().filter(|a| *a != "add" && *a != "import") {
                return Err(format!("fork {}: added is \"add\" or \"import\", not {a:?}", f.fork));
            }
            if f.patches.as_deref() == Some("") {
                return Err(format!("fork {}: patches names a branch, or \"integration\"", f.fork));
            }
        }
        Ok(graph)
    }

    /// The folder a repository's clone is in, relative to the root.
    pub fn clone_path<'a>(&'a self, repo: &'a RepoName) -> &'a str {
        self.clones.get(repo).map(String::as_str).unwrap_or(repo.as_str())
    }
}

/// A source whose files its followers take by a pin file (idealkb's way): the pin names a release,
/// `own` the files the follower keeps for itself, and one file carries a marked part of the source's.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slice {
    /// The repository that gives.
    pub source: RepoName,
    /// The follower's pin file (`version`, `own`).
    pub pin: String,
    /// What a release tag of the source starts with.
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    /// The word in `MARKER:begin` and `MARKER:end` lines around the source's part of `part_file`.
    pub marker: Marker,
    /// The file that holds a marked part.
    #[serde(default = "default_part_file")]
    pub part_file: String,
    /// The follower's file of upstream items, which say a changed own file is on its way up.
    #[serde(default = "default_items")]
    pub items: String,
    /// The pin file's list of files the follower keeps as its own.
    #[serde(default = "default_own_key")]
    pub own_key: String,
    /// The pin file's table of why each own file is kept: a reason (the file is declared own), or
    /// `upstream:ITEM` (it is on its way up, as an upstream item would say).
    #[serde(default = "default_why_key")]
    pub why_key: String,
    /// A command a bump must pass before it is committed (argv, run in the follower's worktree once laid).
    #[serde(default)]
    pub gate: Option<Vec<String>>,
    /// The repositories that follow.
    pub followers: Vec<RepoName>,
}

/// A layer a repository pins by a release of a source, whose contract has one home: the owner, which gives it and reads it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    /// The follower's pin file (`source`, `version` or `template`).
    pub pin: String,
    /// The source when the pin names none.
    pub default_source: RepoName,
    /// The repository that gives the layer and reads its contract: where a follower should pin it.
    pub owner: RepoName,
    /// The owner's file that holds the layer's part.
    pub part: String,
    /// The marker around the layer's part, in the owner's file and in each follower's.
    pub marker: Marker,
    /// What a release tag of the owner starts with.
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    /// The follower's file that carries the part.
    #[serde(default = "default_part_file")]
    pub file: String,
    /// The repositories that follow.
    pub followers: Vec<RepoName>,
}

/// Where a tool's pinned version is written in the follower.
#[derive(Debug, Clone, PartialEq)]
pub enum PinRead {
    /// A dotted key of a TOML file (`tools.lpoco`).
    Key(Vec<String>),
    /// A line of a file: this text before the version and this after it.
    Line {
        /// The line's text before the version.
        before: String,
        /// The line's text after the version.
        after: String,
    },
}

/// A file of the source listing the repositories whose pin a job moves.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listing {
    /// The repository that holds the list.
    pub repo: RepoName,
    /// The file, one repository name per line.
    pub file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTool {
    name: String,
    source: RepoName,
    tag_prefix: String,
    pin: String,
    key: Option<String>,
    line: Option<String>,
    value: Option<String>,
    tag_suffix: Option<String>,
    gate: Option<Vec<String>>,
    #[serde(default)]
    also: Vec<String>,
    listed_in: Option<Listing>,
    followers: Vec<RepoName>,
}

/// A tool pinned at a release of its source.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "RawTool")]
pub struct Tool {
    /// The tool's name, as the report says it.
    pub name: String,
    /// The repository whose tags are its releases.
    pub source: RepoName,
    /// What a release tag starts with.
    pub tag_prefix: String,
    /// The follower's file that holds the pin.
    pub pin: String,
    /// How the version is read from that file.
    pub read: PinRead,
    /// A cutting's releases: tags PREFIX + release + this suffix with its number (`-cut.N`), ordered by release, then N.
    pub tag_suffix: Option<String>,
    /// What a `key` holds around the version (`lpoco-attended-v{version}` for a Cargo git tag): before and after.
    pub value: (String, String),
    /// A command a bump must pass before it is committed (argv, run in the follower's worktree): an attended pin's check.
    pub gate: Option<Vec<String>>,
    /// Files the gate may change, committed with the pin (a Cargo.lock that `cargo check` rewrites for a new tag).
    pub also: Vec<String>,
    /// A list of the repositories whose pin a job moves; a follower not on it moves its pin by hand.
    pub listed_in: Option<Listing>,
    /// The repositories that pin it.
    pub followers: Vec<RepoName>,
}

impl TryFrom<RawTool> for Tool {
    type Error = String;
    fn try_from(r: RawTool) -> Result<Tool, String> {
        let read = match (r.key, r.line) {
            (Some(k), None) => {
                PinRead::Key(key_path(&k).map_err(|e| format!("tool {}: {e}", r.name))?)
            }
            (None, Some(l)) => match l.split_once("{version}") {
                Some((b, a)) => PinRead::Line { before: b.to_string(), after: a.to_string() },
                None => return Err(format!("tool {}: `line` has no {{version}} in it", r.name)),
            },
            _ => return Err(format!("tool {}: give exactly one of `key` and `line`", r.name)),
        };
        let value = match (&read, r.value.as_deref()) {
            (_, None) => (String::new(), String::new()),
            (PinRead::Key(_), Some(v)) => match v.split_once("{version}") {
                Some((b, a)) if !a.contains("{version}") => (b.to_string(), a.to_string()),
                _ => return Err(format!("tool {}: `value` needs one {{version}}", r.name)),
            },
            (PinRead::Line { .. }, Some(_)) => {
                return Err(format!(
                    "tool {}: `value` goes with `key`; a `line` says its own",
                    r.name
                ))
            }
        };
        if let Some(s) = &r.tag_suffix {
            let fits = s.matches('N').count() == 1 && !s.starts_with('N');
            if !fits {
                return Err(format!(
                    "tool {}: tag_suffix has one N, after some text (-cut.N)",
                    r.name
                ));
            }
        }
        if r.gate.as_ref().is_some_and(|g| g.is_empty()) {
            return Err(format!("tool {}: `gate` is a command: argv, not empty", r.name));
        }
        Ok(Tool {
            name: r.name,
            source: r.source,
            tag_prefix: r.tag_prefix,
            pin: r.pin,
            read,
            value,
            tag_suffix: r.tag_suffix,
            gate: r.gate,
            also: r.also,
            listed_in: r.listed_in,
            followers: r.followers,
        })
    }
}

/// A TOML dotted key's parts: `a.b.c`, a part in quotes kept whole (`patch."https://github.com/o/r".pkg.tag`).
fn key_path(key: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut rest = key.trim();
    loop {
        let (part, after) = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let end = rest[1..]
                    .find(q)
                    .ok_or_else(|| format!("key {key:?}: a quote is not closed"))?;
                (rest[1..=end].to_string(), &rest[end + 2..])
            }
            _ => {
                let end = rest.find('.').unwrap_or(rest.len());
                (rest[..end].trim().to_string(), &rest[end..])
            }
        };
        if part.is_empty() {
            return Err(format!("key {key:?} has an empty part"));
        }
        parts.push(part);
        let after = after.trim_start();
        match after.strip_prefix('.') {
            Some(next) => rest = next.trim_start(),
            None if after.is_empty() => return Ok(parts),
            None => return Err(format!("key {key:?}: something after a quoted part")),
        }
    }
}

/// A fork and the upstream its tracking branch should equal.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fork {
    /// The fork.
    pub fork: Remote,
    /// The fork's tracking branch (its HEAD when none).
    pub branch: Option<String>,
    /// Where it came from.
    pub upstream: Remote,
    /// The upstream's branch (its HEAD when none).
    pub upstream_branch: Option<String>,
    /// For a cutting, where its changes live: one branch (`rhizoid/patch` when the cutting names none) or
    /// `"integration"`, the topic branches the repository's `[[integration]]` edge assembles.
    #[serde(default)]
    pub patches: Option<String>,
    /// For a one-branch cutting consumed by its releases (a Cargo git tag): the prefix of the upstream's release tags
    /// (`lpoco-v`). `update ... now` then tags the patch tip RELEASE-cut.N when it moved since its newest cut tag.
    #[serde(default)]
    pub cut_tags: Option<String>,
    /// For an integration cutting, whose releases its topics are on (`torvalds/linux`): what it tracks, in place
    /// of a branch.
    #[serde(default)]
    pub tracks: Option<String>,
    /// For a cutting, how it came to rhizoid: `"add"` (forked by rhizoid) or `"import"` (a fork adopted).
    #[serde(default)]
    pub added: Option<String>,
}

/// A git revision as the graph names it (`origin/main`): something `git rev-parse` reads, never an option.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Rev(String);

impl TryFrom<String> for Rev {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        if s.is_empty()
            || s.starts_with('-')
            || s.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            Err(format!("`{s}` is not a revision"))
        } else {
            Ok(Rev(s))
        }
    }
}

impl std::ops::Deref for Rev {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The word in `MARKER:begin` and `MARKER:end` lines: not empty, or every line would begin a part.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Marker(String);

impl TryFrom<String> for Marker {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        if s.is_empty() || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
            Err(format!("`{s}` is not a marker (a word, no spaces)"))
        } else {
            Ok(Marker(s))
        }
    }
}

impl std::ops::Deref for Marker {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Marker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
