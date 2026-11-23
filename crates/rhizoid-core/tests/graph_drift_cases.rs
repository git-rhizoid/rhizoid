//! The cases of research base-graph's graph-drift.sh, one test each, over a world in memory: what the shell
//! printed `ok` for is `current`, what it printed `FAIL` for is `behind` or `moved-by-hand`.

mod support;

use rhizoid_core::{status, Check, Finding, Level, RepoError, Verdict};
use support::*;

fn run(w: &World) -> Vec<Finding> {
    status(&graph(), w).expect("the world answers")
}

fn of<'a>(fs: &'a [Finding], follower: &str, check: Check) -> Vec<&'a Finding> {
    fs.iter().filter(|f| f.follower == follower && f.check == check).collect()
}

fn one<'a>(fs: &'a [Finding], follower: &str, check: Check) -> &'a Finding {
    let v = of(fs, follower, check);
    assert_eq!(v.len(), 1, "{follower} {check:?}: {v:#?}");
    v[0]
}

#[test]
fn a_world_where_every_edge_is_current_has_no_drift() {
    let fs = run(&World::current());
    let drift: Vec<_> = fs.iter().filter(|f| f.is_drift()).collect();
    assert!(drift.is_empty(), "{drift:#?}");
    assert_eq!(of(&fs, DOCS, Check::SlicePin).len(), 1);
    assert_eq!(fs.iter().filter(|f| f.check == Check::OwnPart).count(), 4);
    assert_eq!(fs.iter().filter(|f| f.check == Check::ToolPin).count(), 5);
    assert_eq!(fs.iter().filter(|f| f.check == Check::ForkTrack).count(), 2);
}

#[test]
fn slice_pin_behind_the_sources_latest_release() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "idealkb.toml", "version = \"0.17.0\"\nown = []\n");
    let fs = run(&w);
    let f = one(&fs, DOCS, Check::SlicePin);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("0.17.0") && f.detail.contains("0.18.0"), "{f}");
    assert_eq!(one(&fs, SURYA, Check::SlicePin).verdict, Verdict::Current);
}

#[test]
fn the_latest_release_is_the_greatest_number_not_the_last_word() {
    let mut w = World::current();
    w.tag(KB, "v0.9.0").tag(KB, "v0.100.0").tag(KB, "vnext");
    let fs = run(&w);
    assert_eq!(one(&fs, DOCS, Check::SlicePin).verdict, Verdict::Behind);
    assert!(one(&fs, DOCS, Check::SlicePin).detail.contains("0.100.0"));
}

#[test]
fn a_pin_naming_a_release_the_source_never_made_is_behind_not_a_crash() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "idealkb.toml", "version = \"9.9.9\"\nown = [\"records/backlog.sql\"]\n");
    let fs = run(&w);
    assert_eq!(one(&fs, DOCS, Check::SlicePin).verdict, Verdict::Behind);
    let f = one(&fs, DOCS, Check::OwnBehind);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("9.9.9"), "{f}");
}

#[test]
fn an_own_file_is_compared_with_the_sources_at_the_pin_not_at_the_latest() {
    let mut w = World::current();
    w.put(KB, "v0.18.0", "records/backlog.sql", "changed after the pin");
    w.put(DOCS, MAIN, "idealkb.toml", "version = \"0.17.0\"\nown = [\"records/backlog.sql\"]\n");
    let fs = run(&w);
    assert_eq!(one(&fs, DOCS, Check::OwnBehind).verdict, Verdict::Current);
}

#[test]
fn an_own_file_changed_by_hand_and_named_by_no_item_is_drift() {
    let mut w = World::current();
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, edited here");
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::OwnBehind);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.is_drift() && f.carried_by.is_none());
    assert!(f.detail.contains("records/backlog.sql"), "{f}");
    assert_eq!(one(&fs, WALLEYE, Check::OwnBehind).verdict, Verdict::Current);
}

#[test]
fn an_own_file_changed_by_hand_but_named_by_an_upstream_item_is_on_its_way_up() {
    let mut w = World::current();
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, edited here");
    w.put(
        SURYA,
        MAIN,
        "data/upstream_items.json",
        "{\"id\":\"other\",\"title\":\"x\",\"note\":null,\"local_ref\":null,\"file\":null}\n\
         {\"id\":\"backlog-rank\",\"title\":\"rank\",\"note\":\"changes records/backlog.sql\",\"local_ref\":null,\"file\":null}\n",
    );
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::OwnBehind);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert_eq!(f.carried_by.as_deref(), Some("backlog-rank"));
    assert!(!f.is_drift());
}

#[test]
fn a_difference_only_in_trailing_newlines_is_no_difference() {
    let mut w = World::current();
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql\n\n");
    assert_eq!(one(&run(&w), SURYA, Check::OwnBehind).verdict, Verdict::Current);
}

#[test]
fn a_tables_file_with_no_marked_part_gets_nothing_new_from_the_source() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "tables.toml", "[tables.own]\n");
    let fs = run(&w);
    let f = one(&fs, DOCS, Check::OwnPart);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.detail.contains("no idealkb part"), "{f}");
}

#[test]
fn a_marked_part_lacking_lines_says_how_many() {
    let mut w = World::current();
    let short = "# idealkb:begin\n[tables.backlog]\n# idealkb:end\n";
    w.put(SURYA, MAIN, "tables.toml", &format!("[tables.own]\n{short}"));
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::OwnPart);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.detail.contains("4 lines"), "{f}");
}

#[test]
fn comments_inside_a_marked_part_are_no_part_of_it() {
    let mut w = World::current();
    let other = KB_PART.replace("# a comment", "# another comment, and more\n# of it");
    w.put(SURYA, MAIN, "tables.toml", &format!("[tables.own]\n{other}"));
    assert_eq!(one(&run(&w), SURYA, Check::OwnPart).verdict, Verdict::Current);
}

#[test]
fn a_layer_pinned_by_a_date_is_behind_and_one_pinned_by_a_release_is_current() {
    let mut w = World::current();
    w.put(
        SURYA,
        MAIN,
        "devicebase.toml",
        &format!("source = \"{DOCS}\"\ntemplate = \"2026-11-17\"\n"),
    );
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::LayerPin);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("2026-11-17"), "{f}");
    assert_eq!(one(&fs, WALLEYE, Check::LayerPin).verdict, Verdict::Current);
}

#[test]
fn a_layer_whose_pin_names_no_source_comes_from_the_default_one_and_that_is_not_the_reader() {
    let mut w = World::current();
    w.put(WALLEYE, MAIN, "devicebase.toml", "template = \"2026-11-17\"\nparts = 1\n");
    let fs = run(&w);
    let f = one(&fs, WALLEYE, Check::LayerOwner);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains(TEMPLATE) && f.detail.contains(DOCS), "{f}");
    assert_eq!(one(&fs, SURYA, Check::LayerOwner).verdict, Verdict::Current);
}

#[test]
fn a_layer_table_declared_outside_the_markers_is_carried_by_hand() {
    let mut w = World::current();
    w.put(
        SURYA,
        MAIN,
        "tables.toml",
        &format!("[tables.devices]\n[tables.parts]\n[tables.own]\n{KB_PART}"),
    );
    let fs = run(&w);
    let hand = of(&fs, SURYA, Check::LayerFollow);
    let tables: Vec<_> = hand.iter().map(|f| f.detail.as_str()).collect();
    assert_eq!(hand.len(), 2, "{tables:?}");
    assert!(hand.iter().all(|f| f.verdict == Verdict::MovedByHand && f.is_drift()));
    assert!(tables[0].contains("devices") && tables[1].contains("parts"), "{tables:?}");
}

#[test]
fn layer_tables_inside_the_markers_are_current_and_tables_of_the_base_are_not_asked() {
    let fs = run(&World::current());
    let f = one(&fs, SURYA, Check::LayerFollow);
    assert_eq!(f.verdict, Verdict::Current);
}

#[test]
fn a_marked_part_that_is_the_one_the_owner_gave_at_the_pinned_release_is_current() {
    let fs = run(&World::current());
    let f = one(&fs, SURYA, Check::LayerEqual);
    assert_eq!(f.verdict, Verdict::Current);
    assert!(f.detail.contains("1.2.0"), "{f}");
}

#[test]
fn a_marked_part_that_differs_from_the_pinned_release_is_moved_by_hand() {
    let mut w = World::current();
    let edited = DEVICE_PART.replace("key = [\"id\"]", "key = [\"serial\"]");
    w.put(SURYA, MAIN, "tables.toml", &format!("{edited}{KB_PART}"));
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::LayerEqual);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.is_drift(), "{f}");
    assert_eq!(one(&fs, WALLEYE, Check::LayerEqual).verdict, Verdict::Current);
}

#[test]
fn an_owner_that_gives_no_layer_is_the_owners_finding() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "device-layer/tables.toml", "nothing here\n");
    let fs = run(&w);
    let f = one(&fs, DOCS, Check::LayerOwner);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("gives no layer"), "{f}");
    assert!(
        of(&fs, SURYA, Check::LayerFollow).is_empty(),
        "nothing to follow: no verdict either way"
    );
}

#[test]
fn a_tool_pinned_behind_its_latest_tag_with_its_own_prefix() {
    let mut w = World::current();
    w.put(LAB_MACHINE, MAIN, "lab.toml", "[tools]\nlpoco = \"0.15.0\"\n");
    let fs = run(&w);
    let f = one(&fs, LAB_MACHINE, Check::ToolPin);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("0.15.0") && f.detail.contains("0.17.1"), "{f}");
}

#[test]
fn a_tool_pinned_in_a_line_of_a_script_is_read_between_the_words_around_it() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "bin/install-tad.sh", "version=\"${TAD_VERSION:-0.6.0}\"\n");
    let fs = run(&w);
    let f = of(&fs, DOCS, Check::ToolPin);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].verdict, Verdict::Behind);
    assert!(f[0].detail.contains("0.6.0") && f[0].detail.contains("0.7.1"), "{}", f[0]);
}

#[test]
fn a_tool_pin_no_job_moves_is_moved_by_hand() {
    let mut w = World::current();
    w.put(TAD, MAIN, "consumers", "# list\nlinux-phone-oco/docs\n");
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::ToolListed);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.is_drift());
    assert_eq!(one(&fs, DOCS, Check::ToolListed).verdict, Verdict::Current);
}

#[test]
fn a_fork_level_with_its_upstream_is_current() {
    let fs = run(&World::current());
    assert_eq!(one(&fs, "grenudi/sxmo-utils", Check::ForkTrack).verdict, Verdict::Current);
    assert_eq!(one(&fs, "grenudi/linux", Check::ForkTrack).verdict, Verdict::Current);
}

#[test]
fn a_clone_that_is_missing_stops_the_report() {
    struct NoClones;
    impl rhizoid_core::Repos for NoClones {
        fn file(
            &self,
            r: &rhizoid_core::RepoName,
            _: &str,
            _: &str,
        ) -> Result<Option<String>, RepoError> {
            Err(RepoError::NoClone(r.to_string()))
        }
        fn tags(&self, r: &rhizoid_core::RepoName) -> Result<Vec<String>, RepoError> {
            Err(RepoError::NoClone(r.to_string()))
        }
        fn head(
            &self,
            _: &rhizoid_core::graph::Remote,
            _: Option<&str>,
        ) -> Result<String, RepoError> {
            Err(RepoError::Failed("offline".into()))
        }
        fn level(&self, _: &rhizoid_core::graph::Remote, _: &str, _: &str) -> rhizoid_core::Level {
            rhizoid_core::Level::Unknown("offline".into())
        }
    }
    let e = status(&graph(), &NoClones).unwrap_err();
    assert!(matches!(e, RepoError::NoClone(_)), "{e}");
}

fn fork_world(level: Level) -> World {
    let mut w = World::current();
    w.set_head("https://git.sr.ht/~mil/sxmo-utils", Some("master"), "1fd7822");
    w.level = Some(level);
    w
}

#[test]
fn a_fork_behind_its_upstream_says_by_how_many_commits() {
    let fs = run(&fork_world(Level::Behind(4)));
    let f = one(&fs, "grenudi/sxmo-utils", Check::ForkTrack);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("4 commits behind"), "{f}");
}

#[test]
fn a_fork_ahead_of_its_upstream_was_changed_by_hand_not_left_behind() {
    let fs = run(&fork_world(Level::Ahead(1)));
    let f = one(&fs, "grenudi/sxmo-utils", Check::ForkTrack);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.is_drift() && f.detail.contains("1 commits ahead"), "{f}");
    let f = one(
        &run(&fork_world(Level::Diverged { behind: 3, ahead: 1 })),
        "grenudi/sxmo-utils",
        Check::ForkTrack,
    )
    .clone();
    assert!(
        f.verdict == Verdict::MovedByHand && f.detail.contains("3 commits behind, 1 ahead"),
        "{f}"
    );
}

#[test]
fn a_fork_whose_distance_the_host_cannot_say_is_behind_and_says_why_not() {
    let fs = run(&fork_world(Level::Unknown("gh api: HTTP 404".into())));
    let f = one(&fs, "grenudi/sxmo-utils", Check::ForkTrack);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("not known") && f.detail.contains("HTTP 404"), "{f}");
}

#[test]
fn a_layer_pinned_to_a_release_the_owner_never_made_is_behind() {
    let mut w = World::current();
    w.put(SURYA, MAIN, "devicebase.toml", &format!("source = \"{DOCS}\"\nversion = \"3.0.0\"\n"));
    let f = run(&w);
    let f = one(&f, SURYA, Check::LayerEqual);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("no release"), "{f}");
}

#[test]
fn a_release_of_the_owner_that_gave_no_part_cannot_be_the_one_carried() {
    let mut w = World::current();
    w.put(DOCS, "v1.2.0", "device-layer/tables.toml", "nothing\n");
    let fs = run(&w);
    assert_eq!(one(&fs, SURYA, Check::LayerEqual).verdict, Verdict::Behind);
}

#[test]
fn a_file_missing_where_a_pin_should_be_is_said_not_read_as_empty() {
    let mut w = World::current();
    w.drop_file(DOCS, MAIN, "idealkb.toml");
    w.drop_file(SURYA, MAIN, "devicebase.toml");
    w.drop_file(WALLEYE, MAIN, "tables.toml");
    w.drop_file(LAB_MACHINE, MAIN, "lab.toml");
    let fs = run(&w);
    assert_eq!(one(&fs, DOCS, Check::SlicePin).verdict, Verdict::Behind);
    assert_eq!(one(&fs, SURYA, Check::LayerPin).verdict, Verdict::Behind);
    assert!(of(&fs, SURYA, Check::LayerOwner).is_empty(), "no pin, no source to judge");
    let f = one(&fs, WALLEYE, Check::LayerFollow);
    assert_eq!(f.verdict, Verdict::Behind);
    assert_eq!(one(&fs, LAB_MACHINE, Check::ToolPin).verdict, Verdict::Behind);
}

#[test]
fn a_tool_pin_file_that_names_no_version_is_behind() {
    let mut w = World::current();
    w.put(LAB_MACHINE, MAIN, "lab.toml", "[tools]\noco-work = \"unknown\"\n");
    let f = run(&w);
    let f = one(&f, LAB_MACHINE, Check::ToolPin);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("names no lpoco version"), "{f}");
}

#[test]
fn an_own_file_that_neither_side_has_is_a_typo_not_current() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "idealkb.toml", "version = \"0.18.0\"\nown = [\"records/typo.sql\"]\n");
    let fs = run(&w);
    let f = one(&fs, DOCS, Check::OwnBehind);
    assert_eq!(f.verdict, Verdict::Behind);
    assert!(f.detail.contains("records/typo.sql"), "{f}");
}

#[test]
fn a_part_the_source_never_had_is_said_so() {
    let mut w = World::current();
    w.put(KB, "v0.18.0", "tables.toml", "[tables.x]\n");
    let fs = run(&w);
    let f = one(&fs, DOCS, Check::OwnPart);
    assert_eq!(f.verdict, Verdict::MovedByHand);
    assert!(f.detail.contains("does not have"), "{f}");
}

fn refused(w: &World) -> String {
    status(&graph(), w).unwrap_err().to_string()
}

#[test]
fn a_pin_file_that_is_not_what_it_should_be_stops_the_report_naming_the_file() {
    let mut w = World::current();
    w.put(DOCS, MAIN, "idealkb.toml", "version = \"0.18.0\"\nown = \"records/x.sql\"\n");
    assert!(refused(&w).contains("`own` is not a list"));
    w.put(DOCS, MAIN, "idealkb.toml", "version = \"0.18.0\"\nown = [1]\n");
    assert!(refused(&w).contains("not a string"));
    w.put(DOCS, MAIN, "idealkb.toml", "version = 18\n");
    assert!(refused(&w).contains("`version` is not a string"));
    w.put(DOCS, MAIN, "idealkb.toml", "version = [[[\n");
    let e = refused(&w);
    assert!(e.contains(DOCS) && e.contains("not valid TOML"), "{e}");
}

#[test]
fn an_items_file_that_is_not_lines_of_json_stops_the_report() {
    let mut w = World::current();
    w.put(SURYA, MAIN, "data/upstream_items.json", "{\"id\":\"a\"}\n[\n  {\n");
    let e = refused(&w);
    assert!(e.contains("upstream_items.json line 2"), "{e}");
}

#[test]
fn a_list_of_pinned_repositories_that_is_missing_stops_the_report() {
    let mut w = World::current();
    w.drop_file(TAD, MAIN, "consumers");
    assert!(refused(&w).contains("consumers"));
}

#[test]
fn a_source_with_no_release_stops_the_report() {
    let mut w = World::current();
    w.drop_tags(TAD);
    assert!(refused(&w).contains("no release tagged tad-v"));
}

#[test]
fn a_clone_that_lacks_the_revision_says_which_clone() {
    let mut w = World::current();
    w.drop_tags(KB);
    w.drop_rev_main(DOCS);
    let e = refused(&w);
    assert!(e.contains(DOCS) || e.contains(KB), "{e}");
}

/// SURYA's pin with a `[why]` table: `own` and the reasons as the follower declares them.
fn surya_why(why: &str) -> World {
    let mut w = World::current();
    w.put(
        SURYA,
        MAIN,
        "idealkb.toml",
        &format!(
            "version = \"0.18.0\"\nown = [\"tables.toml\", \"records/backlog.sql\"]\n\n[why]\n{why}"
        ),
    );
    w
}

#[test]
fn an_own_file_whose_why_gives_a_reason_is_declared_and_current() {
    let mut w = surya_why("\"records/backlog.sql\" = \"the base's records take --project\"\n");
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, the base's own");
    let f = one(&run(&w), SURYA, Check::OwnBehind).clone();
    assert_eq!(f.verdict, Verdict::Current, "{f}");
    assert!(f.detail.contains("declared own: the base's records take --project"), "{f}");
}

#[test]
fn an_own_file_whose_why_names_an_upstream_item_stays_a_candidate_to_go_up() {
    let mut w = surya_why("\"records/backlog.sql\" = \"upstream:backlog-rank\"\n");
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, edited here");
    let f = one(&run(&w), SURYA, Check::OwnBehind).clone();
    assert_eq!(f.verdict, Verdict::MovedByHand, "{f}");
    assert!(!f.detail.contains("no reason"), "{f}");
}

#[test]
fn an_own_file_with_no_reason_in_why_is_still_a_finding() {
    let mut w = surya_why("\"tables.toml\" = \"the base's own tables\"\n");
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, edited here");
    let f = one(&run(&w), SURYA, Check::OwnBehind).clone();
    assert_eq!(f.verdict, Verdict::MovedByHand, "{f}");
    assert!(f.is_drift() && f.detail.contains("no reason in [why]"), "{f}");
}

#[test]
fn an_own_tables_file_with_a_reason_keeps_its_part_as_declared_since_follow_leaves_it_whole() {
    let mut w = surya_why("\"tables.toml\" = \"the base's own tables beside the part\"\n");
    let short = "# idealkb:begin\n[tables.backlog]\n# idealkb:end\n";
    w.put(SURYA, MAIN, "tables.toml", &format!("[tables.own]\n{short}"));
    w.put(DOCS, MAIN, "tables.toml", "[tables.own]\n");
    let fs = run(&w);
    let f = one(&fs, SURYA, Check::OwnPart);
    assert_eq!(f.verdict, Verdict::Current, "{f}");
    assert!(
        f.detail.contains("differs in 4 lines") && f.detail.contains("follow leaves it whole"),
        "{f}"
    );
    // docs gives no reason here: its missing part is still said
    assert_eq!(one(&fs, DOCS, Check::OwnPart).verdict, Verdict::MovedByHand);
}

#[test]
fn a_tables_file_with_no_part_and_a_reason_is_declared() {
    let mut w = surya_why("\"tables.toml\" = \"carries no generic part on purpose\"\n");
    w.put(SURYA, MAIN, "tables.toml", "[tables.own]\n");
    let f = one(&run(&w), SURYA, Check::OwnPart).clone();
    assert_eq!(f.verdict, Verdict::Current, "{f}");
    assert!(f.detail.contains("carries no idealkb part"), "{f}");
}

#[test]
fn the_names_of_own_and_why_come_from_the_slice_edge() {
    let mut graph = graph();
    graph.slices[0].own_key = "kept".into();
    graph.slices[0].why_key = "because".into();
    let mut w = World::current();
    w.put(
        SURYA,
        MAIN,
        "idealkb.toml",
        "version = \"0.18.0\"\nkept = [\"records/backlog.sql\"]\n\n[because]\n\"records/backlog.sql\" = \"ours\"\n",
    );
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, ours");
    let fs = status(&graph, &w).expect("the world answers");
    let f = one(&fs, SURYA, Check::OwnBehind);
    assert_eq!(f.verdict, Verdict::Current, "{f}");
    assert!(f.detail.ends_with("declared own: ours"), "{f}");
}

#[test]
fn a_why_that_is_not_a_table_of_reasons_stops_the_report() {
    let w = surya_why("\"tables.toml\" = 3\n");
    assert!(status(&graph(), &w).is_err());
}

#[test]
fn an_upstream_reason_naming_no_item_is_no_reason() {
    let mut w = surya_why("\"records/backlog.sql\" = \"upstream:\"\n");
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, edited here");
    let f = one(&run(&w), SURYA, Check::OwnBehind).clone();
    assert_eq!(f.verdict, Verdict::MovedByHand, "{f}");
    assert!(f.detail.contains("no reason in [why]"), "{f}");
}

#[test]
fn a_declared_own_file_a_newer_release_changed_says_to_look() {
    let mut w = surya_why("\"records/backlog.sql\" = \"ours\"\n");
    w.put(
        SURYA,
        MAIN,
        "idealkb.toml",
        "version = \"0.17.0\"\nown = [\"tables.toml\", \"records/backlog.sql\"]\n\n[why]\n\"records/backlog.sql\" = \"ours\"\n",
    );
    w.put(KB, "v0.18.0", "records/backlog.sql", "backlog sql, newer");
    w.put(SURYA, MAIN, "records/backlog.sql", "backlog sql, ours");
    let f = one(&run(&w), SURYA, Check::OwnBehind).clone();
    assert_eq!(f.verdict, Verdict::Current, "{f}");
    assert!(f.detail.contains("v0.18.0") || f.detail.contains("0.18.0 changed it"), "{f}");
}

#[test]
fn an_own_tables_file_follow_does_not_read_as_own_is_behind_whatever_its_reason() {
    // a list over several lines: TOML's own, but follow reads only a one-line `own = [...]`, so it lays the part
    let mut w = World::current();
    w.put(
        SURYA,
        MAIN,
        "idealkb.toml",
        "version = \"0.18.0\"\nown = [\n  \"tables.toml\",\n]\n\n[why]\n\"tables.toml\" = \"ours\"\n",
    );
    let short = "# idealkb:begin\n[tables.backlog]\n# idealkb:end\n";
    w.put(SURYA, MAIN, "tables.toml", &format!("[tables.own]\n{short}"));
    let f = one(&run(&w), SURYA, Check::OwnPart).clone();
    assert_eq!(f.verdict, Verdict::Behind, "{f}");
    assert!(f.detail.contains("follow again lays it"), "{f}");
}

#[test]
fn a_why_entry_with_a_dotted_bare_key_is_named_in_the_error() {
    let w = surya_why("tables.toml = \"ours\"\n");
    let e = status(&graph(), &w).unwrap_err().to_string();
    assert!(e.contains("[why] tables") && e.contains("needs quotes"), "{e}");
}
