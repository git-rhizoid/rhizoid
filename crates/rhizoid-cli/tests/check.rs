//! `rhizoid check` on graph files: what this version reads, and a field it does not know refused by name.

use std::process::{Command, Output};

fn check(graph: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rhizoid"))
        .args(["check", "--graph"])
        .arg(graph)
        .output()
        .unwrap()
}

fn example(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
}

#[test]
fn a_graph_this_version_reads_passes_with_what_it_holds() {
    let out = check(&example("linux-phone-oco.graph.toml"));
    let said = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(
        said.contains("readable=yes")
            && said.contains(&format!("rhizoid={}", env!("CARGO_PKG_VERSION")))
    );
    assert!(said.contains("tools=2") && said.contains("forks=2"), "{said}");
}

#[test]
fn a_field_this_version_does_not_know_is_refused_by_name() {
    let dir = std::env::temp_dir().join(format!("rhizoid-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let graph = dir.join("graph.toml");
    let text = std::fs::read_to_string(example("linux-phone-oco.graph.toml")).unwrap();
    std::fs::write(&graph, format!("{text}\n[[later]]\nwhat = \"a field from a newer rhizoid\"\n"))
        .unwrap();
    let out = check(&graph);
    let said = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(said.contains("does not know the field `later`"), "{said}");
    assert!(out.stdout.is_empty(), "nothing on stdout for a graph it cannot read");

    std::fs::write(&graph, text.replacen("[[tool]]\n", "[[tool]]\nmover = \"x\"\n", 1)).unwrap();
    let said = String::from_utf8_lossy(&check(&graph).stderr).to_string();
    assert!(said.contains("does not know the field `mover`"), "an edge's field too: {said}");
}

#[test]
fn a_missing_graph_file_cannot_be_read() {
    let out = check(std::path::Path::new("/nonexistent/graph.toml"));
    assert_eq!(out.status.code(), Some(2), "{out:?}");
}

#[test]
fn a_value_status_or_bump_would_refuse_is_refused_here() {
    let dir = std::env::temp_dir().join(format!("rhizoid-check-values-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let text = std::fs::read_to_string(example("linux-phone-oco.graph.toml")).unwrap();
    let pins =
        "\n[[pins]]\nfollower = \"grenudi/lab-machine\"\nfile = \"lab.toml\"\ntable = \"tools\"\n";
    let graph = dir.join("graph.toml");
    std::fs::write(&graph, format!("{text}{pins}unpinned = {{ oco-work = \"no repository\" }}\n"))
        .unwrap();
    assert_eq!(check(&graph).status.code(), Some(0), "[[pins]] read");
    for (bad, says) in [
        (format!("{text}\n[bump]\nland = []\n"), "[bump] land"),
        (text.replacen("[[tool]]\n", "[[tool]]\nmoved_by = \"\"\n", 1), "moved_by"),
        (format!("{text}{pins}unpinned = {{ oco-work = \"\" }}\n"), "says no why"),
    ] {
        std::fs::write(&graph, bad).unwrap();
        let out = check(&graph);
        assert_eq!(out.status.code(), Some(2), "{says}: {out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(says), "{says}: {out:?}");
    }
}
