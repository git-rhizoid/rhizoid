//! `rhizoid check` - whether this rhizoid can read a graph file: every edge parsed and checked as status and bump
//! read it (`moved_by`, `[[pins]]`, `[bump]` included), with no clones and no network. A field this version does not
//! know is refused, never ignored: an old rhizoid that skipped one could move a pin it does not own. A follower's CI
//! runs it with the rhizoid version its host pins, so a graph never lands ahead of the host.

use rhizoid_core::Graph;
use std::path::Path;

/// Read and check the graph; print what it holds. Err names why this version cannot read it.
pub fn run(graph: &Path, version: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(graph)
        .map_err(|e| format!("cannot read the graph file {}: {e}", graph.display()))?;
    let g = Graph::parse(&text).map_err(|e| refusal(graph, version, &e))?;
    let counts = [
        ("slices", g.slices.len()),
        ("layers", g.layers.len()),
        ("tools", g.tools.len()),
        ("forks", g.forks.len()),
        ("integrations", g.integrations.len()),
        ("pins", g.pin_tables.len()),
    ];
    let mut out = format!("graph={}\nrhizoid={version}\nreadable=yes\n", graph.display());
    for (kind, n) in counts {
        out.push_str(&format!("{kind}={n}\n"));
    }
    crate::commands::status::write_report(&mut std::io::stdout().lock(), &out)
}

/// Why this version cannot read the graph: the parser's words, and for a field it does not know, that field named
/// with what to do.
fn refusal(graph: &Path, version: &str, why: &str) -> String {
    let field = why
        .split_once("unknown field `")
        .and_then(|(_, rest)| rest.split_once('`'))
        .map(|(field, _)| field);
    match field {
        Some(field) => format!(
            "version {version} cannot read {}: it does not know the field `{field}` - a newer rhizoid does, or the field is misspelled\n{}",
            graph.display(),
            why.trim_end()
        ),
        None => format!("version {version} cannot read {}: {}", graph.display(), why.trim_end()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_field_is_named() {
        let why = "TOML parse error at line 3, column 1\n  |\n3 | [[pins]]\n  | ^^^^^^\nunknown field `pins`, expected one of `rev`, `clones`";
        let said = refusal(Path::new("g.toml"), "0.3.6", why);
        assert!(
            said.starts_with("version 0.3.6 cannot read g.toml: it does not know the field `pins`"),
            "{said}"
        );
        assert_eq!(
            refusal(Path::new("g.toml"), "1.0.0", "bad"),
            "version 1.0.0 cannot read g.toml: bad"
        );
    }
}
