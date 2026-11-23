//! Name patterns: the shape a branch or tag name may have, as data a list file or a graph gives - never a
//! project's scheme in code. A pattern is `/`-separated parts, each a literal with an optional CAPITAL
//! placeholder at its end: `N` stands for digits, any other for one part's text (`topic/ITEM`,
//! `backport/ITEM/BASE`, `release/vN`). A placeholder named `BASE` says what the branch is based on.

use serde::Deserialize;

/// One name pattern of a scheme.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Pattern {
    text: String,
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Part {
    literal: String,
    hole: Hole,
    base: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hole {
    None,
    Digits,
    Text,
}

impl TryFrom<String> for Pattern {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        let mut parts = Vec::new();
        for part in text.split('/') {
            let at = part.find(|c: char| c.is_ascii_uppercase()).unwrap_or(part.len());
            let (literal, hole) = part.split_at(at);
            let hole = match hole {
                "" => Hole::None,
                "N" => Hole::Digits,
                h if h.chars().all(|c| c.is_ascii_uppercase() || c == '_') => Hole::Text,
                _ => {
                    return Err(format!("`{text}`: a placeholder is CAPITALS at the end of a part"))
                }
            };
            if literal.is_empty() && hole == Hole::None {
                return Err(format!("`{text}` has an empty part"));
            }
            parts.push(Part { literal: literal.to_string(), hole, base: part[at..] == *"BASE" });
        }
        Ok(Pattern { text, parts })
    }
}

impl Pattern {
    /// Whether `name` has this pattern's shape.
    pub fn matches(&self, name: &str) -> bool {
        let names: Vec<&str> = name.split('/').collect();
        names.len() == self.parts.len()
            && self.parts.iter().zip(names).all(|(p, n)| match n.strip_prefix(p.literal.as_str()) {
                None => false,
                Some(rest) => match p.hole {
                    Hole::None => rest.is_empty(),
                    Hole::Digits => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
                    Hole::Text => !rest.is_empty(),
                },
            })
    }
}

impl Pattern {
    /// The text the pattern was written as.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// What `name` puts in the `BASE` placeholder, when the pattern has one and the name matches.
    pub fn base_of<'n>(&self, name: &'n str) -> Option<&'n str> {
        if !self.matches(name) {
            return None;
        }
        self.parts
            .iter()
            .zip(name.split('/'))
            .find_map(|(p, n)| (p.base && p.hole == Hole::Text).then(|| &n[p.literal.len()..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Pattern {
        Pattern::try_from(s.to_string()).unwrap()
    }

    #[test]
    fn a_pattern_matches_its_shape_only() {
        assert!(p("topic/ITEM").matches("topic/panel"));
        assert!(!p("topic/ITEM").matches("topic/panel/v2"));
        assert!(!p("topic/ITEM").matches("topic/"));
        assert!(p("topic/ITEM/vN").matches("topic/panel/v12"));
        assert!(!p("topic/ITEM/vN").matches("topic/panel/vx"));
        assert!(p("int/DEVICE/LIST/BASE/N").matches("int/walleye/ufs-off/v6.18/3"));
        assert!(p("main").matches("main") && !p("main").matches("master"));
        assert!(Pattern::try_from("topic/ITEMx".to_string()).is_err());
        assert!(Pattern::try_from("topic//x".to_string()).is_err());
    }

    #[test]
    fn a_base_placeholder_says_the_base() {
        assert_eq!(p("backport/ITEM/BASE").base_of("backport/ufs/v6.12"), Some("v6.12"));
        assert_eq!(p("topic/ITEM").base_of("topic/a"), None);
        assert_eq!(p("backport/ITEM/BASE").base_of("topic/a"), None);
    }
}
