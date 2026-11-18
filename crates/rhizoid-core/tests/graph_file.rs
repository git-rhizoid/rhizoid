//! The graph file is read once, at the edge: what it cannot mean it refuses, naming the place.

use rhizoid_core::Graph;

const EXAMPLE: &str = include_str!("../../../examples/linux-phone-oco.graph.toml");

#[test]
fn the_linux_phone_oco_graph_parses_with_every_edge_kind() {
    let g = Graph::parse(EXAMPLE).unwrap();
    assert_eq!((g.slices.len(), g.layers.len(), g.tools.len(), g.forks.len()), (1, 1, 2, 2));
    assert_eq!(&*g.rev, "origin/main");
    assert_eq!(g.slices[0].followers.len(), 4);
}

#[test]
fn a_clone_is_where_its_name_says_unless_the_graph_says_otherwise() {
    let g = Graph::parse(EXAMPLE).unwrap();
    let kb = "grenudi/ideal-knowledge-base".to_string().try_into().unwrap();
    let lm = "grenudi/lab-machine".to_string().try_into().unwrap();
    assert_eq!(g.clone_path(&kb), "grenudi/ideal-knowledge-base");
    assert_eq!(g.clone_path(&lm), "lab-machine");
}

#[test]
fn a_misspelled_key_is_an_error_not_a_default() {
    let e = Graph::parse("revv = \"x\"\n").unwrap_err();
    assert!(e.contains("revv"), "{e}");
}

#[test]
fn a_name_that_is_not_owner_slash_repo_is_refused() {
    let text = "[[fork]]\nfork = \"just-a-name\"\nupstream = \"a/b\"\n";
    let e = Graph::parse(text).unwrap_err();
    assert!(e.contains("just-a-name"), "{e}");
}

#[test]
fn a_tool_pin_is_a_key_or_a_line_never_both_never_neither() {
    let base = "[[tool]]\nname = \"t\"\nsource = \"a/b\"\ntag_prefix = \"v\"\npin = \"f\"\nfollowers = [\"a/c\"]\n";
    assert!(Graph::parse(base).unwrap_err().contains("exactly one"));
    let both = format!("{base}key = \"x\"\nline = \"v={{version}}\"\n");
    assert!(Graph::parse(&both).unwrap_err().contains("exactly one"));
    let bad = format!("{base}line = \"no placeholder\"\n");
    assert!(Graph::parse(&bad).unwrap_err().contains("{version}"));
    assert!(Graph::parse(&format!("{base}key = \"tools.x\"\n")).is_ok());
}

#[test]
fn nothing_in_the_graph_can_become_an_option_of_git_or_leave_the_root() {
    let fork = |f: &str| format!("[[fork]]\nfork = \"{f}\"\nupstream = \"a/b\"\n");
    let e = Graph::parse(&fork("--upload-pack=touch /x ://")).unwrap_err();
    assert!(e.contains("starts with a dash"), "{e}");
    assert!(Graph::parse("rev = \"--output=/x\"\n").unwrap_err().contains("not a revision"));
    assert!(Graph::parse(&fork("a/..")).is_err());
    assert!(Graph::parse("[clones]\n\"a/b\" = \"../../etc\"\n").unwrap_err().contains("no `..`"));
    assert!(Graph::parse("[clones]\n\"a/b\" = \"/etc\"\n").is_err());
}

#[test]
fn a_marker_is_a_word_or_every_line_would_begin_a_part() {
    let slice = |m: &str| {
        format!("[[slice]]\nsource = \"a/b\"\npin = \"p.toml\"\nmarker = \"{m}\"\nfollowers = [\"a/c\"]\n")
    };
    assert!(Graph::parse(&slice("")).unwrap_err().contains("not a marker"));
    assert!(Graph::parse(&slice("two words")).is_err());
    assert!(Graph::parse(&slice("kb")).is_ok());
}
