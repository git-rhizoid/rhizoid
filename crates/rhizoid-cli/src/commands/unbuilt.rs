//! The commands that wait for the GitHub adapter: forking, adopting, updating and refreshing a cutting need
//! `rhizoid-core::ports`' GitHubPort, which is step 4 of the order the author unparked git-rhizoid with
//! (status, follow, send-back, cuttings). Until then each says so rather than succeed silently.

/// A command that is not built yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Add,
    Import,
    Update,
    Refresh,
}

impl Verb {
    fn word(self) -> &'static str {
        match self {
            Verb::Add => "add",
            Verb::Import => "import",
            Verb::Update => "update",
            Verb::Refresh => "refresh",
        }
    }

    fn waits_for(self) -> &'static str {
        match self {
            Verb::Add => "forking needs the GitHubPort adapter, which is out of scope for the boundary layer (see rhizoid-core::ports)",
            Verb::Import => "needs the GitHubPort adapter to confirm the fork exists and is reachable",
            Verb::Update => "needs the GitHubPort adapter's merge_upstream",
            Verb::Refresh => "needs the GitHubPort adapter to observe the fork's real state",
        }
    }
}

/// Why `verb` (about `target`, if given) does nothing yet.
pub fn run(verb: Verb, target: Option<&str>) -> Result<(), String> {
    let word = verb.word();
    Err(match target {
        Some(t) => format!("rhizoid {word} {t}: not implemented yet - {}", verb.waits_for()),
        None => format!("rhizoid {word}: not implemented yet - {}", verb.waits_for()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_unbuilt_command_says_what_it_waits_for() {
        for v in [Verb::Add, Verb::Import, Verb::Update, Verb::Refresh] {
            let e = run(v, Some("x/y")).unwrap_err();
            assert!(e.contains("not implemented yet") && e.contains("x/y"), "{e}");
            assert!(e.contains(v.word()) && e.contains("GitHubPort"), "{e}");
        }
    }

    #[test]
    fn a_command_with_no_target_says_so_without_one() {
        assert!(run(Verb::Update, None)
            .unwrap_err()
            .starts_with("rhizoid update: not implemented"));
    }
}
