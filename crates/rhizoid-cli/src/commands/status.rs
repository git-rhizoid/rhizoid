//! `rhizoid status` - every edge of a repository graph against what its source gives, read-only.
//!
//! The graph file names the edges; each follower's own pin files, read at the graph's revision of its clone,
//! say what it follows. Each check is `current`, `behind` or `moved-by-hand`, and how. It ends 0 when nothing
//! is drift, 1 when something is, 2 when the world could not be read.

use crate::git_repos::GitRepos;
use rhizoid_core::{drift_count, kernel_status, status, summary, Finding, Graph};
use std::path::{Path, PathBuf};

/// The graph file read when none is named.
pub const GRAPH_FILE: &str = "rhizoid.graph.toml";

/// What status was asked, with every default already decided.
pub struct Ask {
    pub graph: PathBuf,
    pub root: PathBuf,
    pub fetch: bool,
    pub json: bool,
}

/// The folder the clones are under when none is named: `~/Projects`.
pub fn default_root(home: Option<&str>) -> Result<PathBuf, String> {
    match home {
        Some(h) if !h.is_empty() => Ok(Path::new(h).join("Projects")),
        _ => Err("no --root, no RHIZOID_ROOT and no HOME to find ~/Projects from".to_string()),
    }
}

/// The report: a line per finding and a summary, or the findings as JSON.
pub fn render(findings: &[Finding], json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(findings).expect("findings serialise") + "\n";
    }
    let mut out = String::new();
    for f in findings {
        out.push_str(&f.to_string());
        out.push('\n');
    }
    out.push_str(&summary(findings));
    out.push('\n');
    out
}

/// Print the report; whether anything was drift.
pub fn run(ask: &Ask) -> Result<bool, String> {
    let text = std::fs::read_to_string(&ask.graph)
        .map_err(|e| format!("cannot read the graph file {}: {e}", ask.graph.display()))?;
    let graph = Graph::parse(&text).map_err(|e| format!("{}: {e}", ask.graph.display()))?;
    let repos = GitRepos::new(&ask.root, &graph, ask.fetch);
    let mut findings = status(&graph, &repos).map_err(|e| e.to_string())?;
    findings.extend(kernel_status(&graph.kernels, &repos).map_err(|e| e.to_string())?);
    write_report(&mut std::io::stdout().lock(), &render(&findings, ask.json))?;
    Ok(drift_count(&findings) > 0)
}

/// Write the report. A reader that went away (`rhizoid status | head`) is not an error of the report.
pub fn write_report(out: &mut impl std::io::Write, report: &str) -> Result<(), String> {
    match out.write_all(report.as_bytes()).and_then(|()| out.flush()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            Err(format!("cannot write the report: {e}"))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhizoid_core::{Check, Verdict};

    fn finding(verdict: Verdict) -> Finding {
        Finding {
            follower: "a/b".into(),
            source: "c/d".into(),
            check: Check::ToolPin,
            verdict,
            carried_by: None,
            detail: "tad 0.7.1".into(),
        }
    }

    #[test]
    fn the_text_report_is_a_line_per_finding_then_the_summary() {
        let r = render(&[finding(Verdict::Current), finding(Verdict::Behind)], false);
        let lines: Vec<&str> = r.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("current") && lines[1].starts_with("behind"));
        assert!(lines[2].contains("1 drift"));
    }

    #[test]
    fn the_json_report_is_the_findings() {
        let r = render(&[finding(Verdict::Behind)], true);
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v[0]["follower"], "a/b");
        assert_eq!(v[0]["verdict"], "behind");
    }

    struct Closed(std::io::ErrorKind);
    impl std::io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(self.0.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_reader_that_went_away_is_no_error_and_a_full_disk_is() {
        assert!(write_report(&mut Closed(std::io::ErrorKind::BrokenPipe), "x").is_ok());
        assert!(write_report(&mut Closed(std::io::ErrorKind::PermissionDenied), "x").is_err());
    }

    #[test]
    fn the_root_is_projects_under_home() {
        assert_eq!(default_root(Some("/home/x")).unwrap(), PathBuf::from("/home/x/Projects"));
        assert!(default_root(None).is_err());
        assert!(default_root(Some("")).is_err());
    }
}
