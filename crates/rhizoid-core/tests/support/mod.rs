//! A world in memory: the repositories of the linux-phone-oco graph as `graph-drift.sh` read them, every edge current.
//! A test changes one thing and asks what `status` says - the shell's cases as fixtures.

use rhizoid_core::graph::{Remote, RepoName};
use rhizoid_core::{Graph, Level, RepoError, Repos};
use std::collections::{BTreeMap, BTreeSet};

pub const GRAPH: &str = include_str!("../../../../examples/linux-phone-oco.graph.toml");
pub const KB: &str = "grenudi/ideal-knowledge-base";
pub const DOCS: &str = "linux-phone-oco/docs";
pub const TEMPLATE: &str = "linux-phone-oco/device-template";
pub const SURYA: &str = "linux-phone-oco/surya-lab";
pub const WALLEYE: &str = "linux-phone-oco/google-walleye-lab";
pub const BASES: [&str; 4] = [DOCS, TEMPLATE, SURYA, WALLEYE];
pub const LAB_TOOLS: &str = "linux-phone-oco/lab-tools";
pub const LAB_MACHINE: &str = "grenudi/lab-machine";
pub const TAD: &str = "creation-guidelines/tad-engine";
pub const MAIN: &str = "origin/main";

/// The marked part idealkb gives, as a base's tables.toml carries it.
pub const KB_PART: &str = "# idealkb:begin - the standing tables\n[tables.backlog]\nkey = [\"id\"]\n\n# a comment\n[tables.decisions]\nkey = [\"id\"]\n# idealkb:end\n";
pub const DEVICE_PART: &str = "# device:begin - the device layer, kept by docs\n[tables.devices]\nkey = [\"id\"]\n[tables.parts]\n[tables.test_runs]\n[tables.test_results]\n# device:end\n";

#[derive(Default, Clone)]
pub struct World {
    files: BTreeMap<(String, String, String), String>,
    tags: BTreeMap<String, BTreeSet<String>>,
    heads: BTreeMap<(String, Option<String>), String>,
    pub level: Option<Level>,
    no_main: BTreeSet<String>,
    reference_tags: BTreeMap<(String, String), String>,
}

impl World {
    pub fn put(&mut self, repo: &str, rev: &str, path: &str, text: &str) -> &mut Self {
        self.files.insert((repo.into(), rev.into(), path.into()), text.into());
        self
    }
    pub fn drop_file(&mut self, repo: &str, rev: &str, path: &str) -> &mut Self {
        self.files.remove(&(repo.into(), rev.into(), path.into()));
        self
    }
    pub fn drop_tags(&mut self, repo: &str) -> &mut Self {
        self.tags.remove(repo);
        self
    }
    pub fn drop_rev_main(&mut self, repo: &str) -> &mut Self {
        self.no_main.insert(repo.to_string());
        self
    }
    pub fn reference_tag(&mut self, repo: &str, tag: &str, commit: &str) -> &mut Self {
        self.reference_tags.insert((repo.into(), tag.into()), commit.into());
        self
    }
    pub fn tag(&mut self, repo: &str, tag: &str) -> &mut Self {
        self.tags.entry(repo.into()).or_default().insert(tag.into());
        self
    }
    pub fn set_head(&mut self, url: &str, branch: Option<&str>, sha: &str) -> &mut Self {
        self.heads.insert((url.into(), branch.map(Into::into)), sha.into());
        self
    }

    /// Every edge of the linux-phone-oco graph carries what its source gives.
    pub fn current() -> World {
        let mut w = World::default();
        for tag in ["v0.17.0", "v0.18.0"] {
            w.tag(KB, tag);
            w.put(KB, tag, "tables.toml", KB_PART);
            w.put(KB, tag, "records/backlog.sql", "backlog sql");
        }
        w.put(KB, MAIN, "tables.toml", KB_PART);
        for b in BASES {
            w.put(
                b,
                MAIN,
                "idealkb.toml",
                "version = \"0.18.0\"\nown = [\"tables.toml\", \"records/backlog.sql\"]\n",
            );
            w.put(b, MAIN, "tables.toml", &format!("[tables.own]\n{KB_PART}"));
            w.put(b, MAIN, "records/backlog.sql", "backlog sql");
            w.put(
                b,
                MAIN,
                "bin/install-tad.sh",
                "#!/bin/sh\nversion=\"${TAD_VERSION:-0.7.1}\"\necho $version\n",
            );
        }
        for b in [SURYA, WALLEYE, TEMPLATE] {
            w.put(
                b,
                MAIN,
                "devicebase.toml",
                &format!("source = \"{DOCS}\"\nversion = \"1.2.0\"\nparts = 1\n"),
            );
        }
        for b in [SURYA, WALLEYE] {
            w.put(b, MAIN, "tables.toml", &format!("[tables.own]\n{DEVICE_PART}{KB_PART}"));
        }
        w.put(TEMPLATE, MAIN, "tables.toml", &format!("{DEVICE_PART}{KB_PART}"));
        w.tag(DOCS, "v1.2.0").put(DOCS, "v1.2.0", "device-layer/tables.toml", DEVICE_PART);
        w.put(DOCS, MAIN, "device-layer/tables.toml", DEVICE_PART);
        w.tag(LAB_TOOLS, "lpoco-v0.17.0")
            .tag(LAB_TOOLS, "lpoco-v0.17.1")
            .tag(LAB_TOOLS, "other-v9.9.9");
        w.put(
            LAB_MACHINE,
            MAIN,
            "lab.toml",
            "root = \"sudo\"\n[tools]\nlpoco = \"0.17.1\"\noco-work = \"unknown\"\n",
        );
        w.tag(TAD, "tad-v0.7.0").tag(TAD, "tad-v0.7.1");
        w.put(TAD, MAIN, "consumers", "# the repositories that use tad\n\nlinux-phone-oco/docs\nlinux-phone-oco/device-template\nlinux-phone-oco/surya-lab\nlinux-phone-oco/google-walleye-lab\nelsewhere/repo\n");
        w.set_head("https://github.com/grenudi/sxmo-utils", Some("master"), "303c755");
        w.set_head("https://git.sr.ht/~mil/sxmo-utils", Some("master"), "303c755");
        w.set_head("https://github.com/grenudi/linux", None, "aaaa111");
        w.set_head("https://github.com/sm7150-mainline/linux", None, "aaaa111");
        w
    }
}

impl Repos for World {
    fn file(&self, repo: &RepoName, rev: &str, path: &str) -> Result<Option<String>, RepoError> {
        let known = (rev == MAIN && !self.no_main.contains(repo.as_str()))
            || self.tags.get(repo.as_str()).is_some_and(|t| t.contains(rev));
        if !known {
            return Err(RepoError::NoRev { repo: repo.to_string(), rev: rev.to_string() });
        }
        Ok(self.files.get(&(repo.to_string(), rev.to_string(), path.to_string())).cloned())
    }
    fn tags(&self, repo: &RepoName) -> Result<Vec<String>, RepoError> {
        Ok(self.tags.get(repo.as_str()).map(|t| t.iter().cloned().collect()).unwrap_or_default())
    }
    fn head(&self, remote: &Remote, branch: Option<&str>) -> Result<String, RepoError> {
        self.heads
            .get(&(remote.url(), branch.map(Into::into)))
            .cloned()
            .ok_or_else(|| RepoError::Failed(format!("{} has no {branch:?}", remote.url())))
    }
    fn level(&self, _: &Remote, _: &str, _: &str) -> Level {
        self.level.clone().unwrap_or_else(|| Level::Unknown("the fixture says nothing".into()))
    }
    fn grep(
        &self,
        repo: &RepoName,
        rev: &str,
        text: &str,
    ) -> Result<Vec<(String, u64, String)>, RepoError> {
        self.file(repo, rev, "")?;
        let mut hits = Vec::new();
        for ((r, v, path), body) in &self.files {
            if r == repo.as_str() && v == rev {
                for (n, line) in body.lines().enumerate() {
                    if line.contains(text) {
                        hits.push((path.clone(), n as u64 + 1, line.to_string()));
                    }
                }
            }
        }
        Ok(hits)
    }
    fn reference(
        &self,
        r: &rhizoid_core::graph::Reference,
    ) -> Result<rhizoid_core::RefRead, RepoError> {
        Ok(match self.reference_tags.get(&(r.repo.to_string(), r.tag.clone())) {
            Some(c) => rhizoid_core::RefRead::At(c.clone()),
            None => rhizoid_core::RefRead::Missing,
        })
    }
    /// The fixture's .gitattributes, read for the two shapes the tests use: `DIR/** ATTR` and `PATH ATTR`.
    fn marked(
        &self,
        repo: &RepoName,
        rev: &str,
        attr: &str,
        paths: &[String],
    ) -> Result<BTreeSet<String>, RepoError> {
        let attributes = self.file(repo, rev, ".gitattributes")?.unwrap_or_default();
        let patterns: Vec<&str> = attributes
            .lines()
            .filter_map(|l| l.split_once(' ').filter(|(_, a)| a.trim() == attr).map(|(p, _)| p))
            .collect();
        Ok(paths
            .iter()
            .filter(|path| {
                patterns.iter().any(|p| match p.strip_suffix("/**") {
                    Some(dir) => path.starts_with(&format!("{dir}/")),
                    None => *path == p,
                })
            })
            .cloned()
            .collect())
    }
}

pub fn graph() -> Graph {
    Graph::parse(GRAPH).expect("the example graph parses")
}
