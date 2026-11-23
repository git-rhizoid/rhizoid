//! What `status` says: one finding per edge and what was checked on it.

use serde::Serialize;
use std::fmt;

/// How an edge stands against its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// It carries what its source gives.
    Current,
    /// Its source has moved on, or it was never pinned to something that can move.
    Behind,
    /// The follower changed what the source gives, or keeps by hand what the source would give.
    MovedByHand,
    /// What was read cannot say yet; never drift.
    NotKnownYet,
}

impl Verdict {
    /// The word the report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Current => "current",
            Verdict::Behind => "behind",
            Verdict::MovedByHand => "moved-by-hand",
            Verdict::NotKnownYet => "not-known-yet",
        }
    }
}

/// What was checked on an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    /// A slice pin against the source's latest release.
    SlicePin,
    /// A file the follower keeps as its own, against the source's at the pin.
    OwnBehind,
    /// The follower's copy of the source's marked part of a file.
    OwnPart,
    /// A layer pinned by a release, not a date.
    LayerPin,
    /// A layer's source is the repository that reads its contract.
    LayerOwner,
    /// A layer's tables sit in the marked part the owner gives, not beside it.
    LayerFollow,
    /// A follower's marked part is the one the owner gave at the release the follower pins.
    LayerEqual,
    /// A tool pin against the source's latest release.
    ToolPin,
    /// The follower is on the list of repositories whose tool pin a job moves.
    ToolListed,
    /// A fork's tracking branch against its upstream's head.
    ForkTrack,
    /// A repository's branch and tag names against the scheme its graph gives.
    NameScheme,
    /// A topic against the newest release, a backport against the base its name says.
    TopicBase,
    /// A local topic branch against `origin`'s of the same name.
    BranchOrigin,
    /// An assembled tag's recipe, read back from its message, against the branches now.
    Recipe,
    /// An assembled worktree against its tag.
    WorktreeTag,
}

impl Check {
    /// The name the report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Check::SlicePin => "slice-pin",
            Check::OwnBehind => "own-behind",
            Check::OwnPart => "own-part",
            Check::LayerPin => "layer-pin",
            Check::LayerOwner => "layer-owner",
            Check::LayerFollow => "layer-follow",
            Check::LayerEqual => "layer-equal",
            Check::ToolPin => "tool-pin",
            Check::ToolListed => "tool-listed",
            Check::ForkTrack => "fork-track",
            Check::NameScheme => "name-scheme",
            Check::TopicBase => "topic-base",
            Check::BranchOrigin => "branch-origin",
            Check::Recipe => "recipe",
            Check::WorktreeTag => "worktree-tag",
        }
    }
}

/// One edge's one check, with its verdict and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// The follower: the repository that takes.
    pub follower: String,
    /// The source: where it takes from.
    pub source: String,
    /// What was checked.
    pub check: Check,
    /// How it stands.
    pub verdict: Verdict,
    /// For a change made by hand: the upstream item that says it is on its way up.
    pub carried_by: Option<String>,
    /// How, in words.
    pub detail: String,
}

impl Finding {
    /// Whether this is drift nothing yet answers: behind, or moved by hand with no item carrying it.
    pub fn is_drift(&self) -> bool {
        match self.verdict {
            Verdict::Current | Verdict::NotKnownYet => false,
            Verdict::Behind => true,
            Verdict::MovedByHand => self.carried_by.is_none(),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<13} {} <- {}  {}: {}",
            self.verdict.as_str(),
            self.follower,
            self.source,
            self.check.as_str(),
            self.detail
        )?;
        if let Some(item) = &self.carried_by {
            write!(f, " (on its way up: {item})")?;
        }
        Ok(())
    }
}

/// The edges that are drift: nothing yet carries them up or brings them level.
pub fn drift_count(findings: &[Finding]) -> usize {
    findings.iter().filter(|f| f.is_drift()).count()
}

/// One line for the end of a report: how many findings, and how they stand.
pub fn summary(findings: &[Finding]) -> String {
    let n = |v: Verdict| findings.iter().filter(|f| f.verdict == v).count();
    let unknown = match n(Verdict::NotKnownYet) {
        0 => String::new(),
        k => format!(", {k} not known yet"),
    };
    format!(
        "{} checks: {} current, {} behind, {} moved by hand{unknown} ({} drift)",
        findings.len(),
        n(Verdict::Current),
        n(Verdict::Behind),
        n(Verdict::MovedByHand),
        drift_count(findings)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(verdict: Verdict, carried_by: Option<&str>) -> Finding {
        Finding {
            follower: "a/b".into(),
            source: "c/d".into(),
            check: Check::SlicePin,
            verdict,
            carried_by: carried_by.map(Into::into),
            detail: "how".into(),
        }
    }

    #[test]
    fn a_change_made_by_hand_is_drift_unless_an_item_carries_it() {
        assert!(!f(Verdict::Current, None).is_drift());
        assert!(f(Verdict::Behind, None).is_drift());
        assert!(f(Verdict::MovedByHand, None).is_drift());
        assert!(!f(Verdict::MovedByHand, Some("item")).is_drift());
    }

    #[test]
    fn the_line_says_who_follows_whom_how_and_what_carries_it() {
        let line = f(Verdict::MovedByHand, Some("item")).to_string();
        assert_eq!(line, "moved-by-hand a/b <- c/d  slice-pin: how (on its way up: item)");
    }

    #[test]
    fn the_summary_counts_each_verdict_and_the_drift() {
        let all = [
            f(Verdict::Current, None),
            f(Verdict::Behind, None),
            f(Verdict::MovedByHand, Some("i")),
        ];
        assert_eq!(summary(&all), "3 checks: 1 current, 1 behind, 1 moved by hand (1 drift)");
    }

    #[test]
    fn json_names_the_verdict_and_the_check_as_the_report_does() {
        let j = serde_json::to_value(f(Verdict::MovedByHand, None)).unwrap();
        assert_eq!(j["verdict"], "moved-by-hand");
        assert_eq!(j["check"], "slice-pin");
        assert_eq!(j["carried_by"], serde_json::Value::Null);
    }
}
