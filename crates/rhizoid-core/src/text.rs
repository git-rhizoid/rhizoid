//! The small readings `status` makes of files: pure functions of text, each pinned by its test.

/// The non-comment lines between `MARKER:begin` and `MARKER:end` lines, the marker lines themselves included
/// when they are not comments. A file may hold the part more than once.
pub fn marked_part<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
    let begin = format!("{marker}:begin");
    let end = format!("{marker}:end");
    let mut on = false;
    let mut part = Vec::new();
    for line in text.lines() {
        if line.contains(&begin) {
            on = true;
        }
        if on && !line.trim_start().starts_with('#') {
            part.push(line);
        }
        if line.contains(&end) {
            on = false;
        }
    }
    // as the shell reads a part (`$(...)`), blank lines at its end are no part of it
    while part.last() == Some(&"") {
        part.pop();
    }
    part
}

/// Every line from a `MARKER:begin` line to the next `MARKER:end` line, both included, comments as well:
/// what a follower must carry unchanged.
pub fn raw_marked_part<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
    let begin = format!("{marker}:begin");
    let end = format!("{marker}:end");
    let mut on = false;
    let mut part = Vec::new();
    for line in text.lines() {
        if line.contains(&begin) {
            on = true;
        }
        if on {
            part.push(line);
        }
        if line.contains(&end) {
            on = false;
        }
    }
    part
}

/// The names of the tables a file declares: lines that start `[tables.NAME]`, NAME of `a-z` and `_`.
pub fn table_names<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    lines
        .into_iter()
        .filter_map(|l| {
            let name = l.strip_prefix("[tables.")?.split_once(']')?.0;
            (!name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
                .then(|| name.to_string())
        })
        .collect()
}

/// How many lines of `want` are not in the longest run of lines `want` and `have` share in order:
/// what `diff want have` would mark as taken away from `want`.
pub fn lines_missing(want: &[&str], have: &[&str]) -> usize {
    let mut row = vec![0usize; have.len() + 1];
    for w in want {
        let mut diag = 0;
        for (j, h) in have.iter().enumerate() {
            let up = row[j + 1];
            row[j + 1] = if w == h { diag + 1 } else { row[j + 1].max(row[j]) };
            diag = up;
        }
    }
    want.len() - row[have.len()]
}

/// Whether two files are the same as a shell's `$(...)` sees them: trailing newlines do not count.
pub fn same_text(a: &str, b: &str) -> bool {
    a.trim_end_matches('\n') == b.trim_end_matches('\n')
}

/// A release number: dot-separated whole numbers after a prefix, ordered as numbers.
pub fn release_number(tag: &str, prefix: &str) -> Option<Vec<u64>> {
    let rest = tag.strip_prefix(prefix)?;
    rest.split('.').map(|p| p.parse::<u64>().ok()).collect()
}

/// The greatest release of `tags` that starts with `prefix`, as its number without the prefix.
pub fn latest_release(tags: &[String], prefix: &str) -> Option<String> {
    tags.iter()
        .filter_map(|t| release_number(t, prefix).map(|n| (n, t)))
        .max()
        .map(|(_, t)| t[prefix.len()..].to_string())
}

/// `1.2.3`: three whole numbers, a release and not a date.
pub fn is_three_part_release(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marked_part_skips_comments_and_stops_at_the_end_marker() {
        let t = "a\n# x:begin note\nb\n  # c\nd\n# x:end\ne\n";
        assert_eq!(marked_part(t, "x"), vec!["b", "d"]);
        assert!(marked_part(t, "y").is_empty());
    }

    #[test]
    fn blank_lines_at_the_end_of_a_part_are_no_part_of_it() {
        let t = "# x:begin\n1\n\n\n# x:end\n";
        assert_eq!(marked_part(t, "x"), vec!["1"]);
    }

    #[test]
    fn the_raw_part_keeps_the_comments_and_both_marker_lines() {
        let t = "a\n# x:begin note\nb\n# c\n# x:end\ne\n";
        assert_eq!(raw_marked_part(t, "x"), vec!["# x:begin note", "b", "# c", "# x:end"]);
    }

    #[test]
    fn a_marked_part_may_come_twice() {
        let t = "# x:begin\n1\n# x:end\n0\n# x:begin\n2\n# x:end\n";
        assert_eq!(marked_part(t, "x"), vec!["1", "2"]);
    }

    #[test]
    fn table_names_are_the_headers_and_nothing_nested() {
        let l = [
            "[tables.devices]",
            "[tables.test_runs]",
            "[[tables.devices.fields]]",
            "[tables.Bad]",
            "x [tables.a]",
        ];
        assert_eq!(table_names(l), vec!["devices", "test_runs"]);
    }

    #[test]
    fn lines_missing_counts_what_diff_would_take_away() {
        assert_eq!(lines_missing(&["a", "b", "c"], &["a", "c"]), 1);
        assert_eq!(lines_missing(&["a", "b", "c"], &["c", "b", "a"]), 2);
        assert_eq!(lines_missing(&["a", "b"], &["a", "b", "extra"]), 0);
        assert_eq!(lines_missing(&["a", "b"], &[]), 2);
    }

    #[test]
    fn trailing_newlines_do_not_make_two_files_differ() {
        assert!(same_text("a\n\n", "a"));
        assert!(!same_text("a\n", "b\n"));
    }

    #[test]
    fn releases_order_as_numbers() {
        let tags: Vec<String> =
            ["v0.9.0", "v0.10.0", "vnext", "w9.9.9", "v0.10"].map(String::from).to_vec();
        assert_eq!(latest_release(&tags, "v").as_deref(), Some("0.10.0"));
        assert_eq!(latest_release(&tags, "x"), None);
    }

    #[test]
    fn a_release_has_three_numbers_and_a_date_does_not() {
        assert!(is_three_part_release("0.17.1"));
        assert!(!is_three_part_release("2026-11-17"));
        assert!(!is_three_part_release("1.2"));
        assert!(!is_three_part_release("1.2.x"));
    }
}
