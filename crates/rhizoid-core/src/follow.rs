//! `follow`: lay a release of a source into a base, exactly as ideal-knowledge-base's `bin/follow.sh` does - the
//! files its `generic` list names, as that release has them; the part of a file between the marker lines; the
//! base's own files left as they are; the pin moved. What it prints and the files it writes are the shell's, byte
//! for byte (rhizoid's tests/agree-with-follow.sh runs both and compares), so it can stand in for it.
//!
//! The shell's ways are kept where they show: a comparison of two texts ignores their trailing newlines (a shell
//! command substitution drops them); `generic` lines
//! split on whitespace; marker lines are found by `MARK:begin` and `MARK:end`; a file the list names that the
//! release lacks ends it there, with git's exit code 128, as `set -e` ends the shell.

use crate::ports::{CommandPort, GitPort, PortError};
use std::path::Path;

/// What one follow said and how it ended: 0 when the base had no change of its own to a generic file, 3 when it
/// had (the change shows in the diff, the signal to send it upstream), 2 when it could not run, 1 when the pin did
/// not move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followed {
    /// What it printed on stdout.
    pub said: String,
    /// What it printed on stderr.
    pub complained: String,
    /// Its exit code.
    pub code: i32,
}

/// Why follow could not run: the shell's stderr and exit code.
fn refused(said: String, complaint: impl Into<String>, code: i32) -> Followed {
    Followed { said, complained: complaint.into() + "\n", code }
}

/// Lay release `version` of the source clone `git` (whose folder is `source`) into `base`.
pub fn follow(
    git: &impl GitPort,
    cmd: &impl CommandPort,
    source: &Path,
    base: &Path,
    version: &str,
) -> Result<Followed, PortError> {
    let tag = format!("v{version}");
    if git.commit(&tag)?.is_none() {
        return Ok(refused(
            String::new(),
            format!("follow: no release {tag} in {}", source.display()),
            2,
        ));
    }
    let at = |v: &str, path: &str| -> Result<Vec<u8>, PortError> {
        Ok(git.show(&format!("v{v}"), path)?.unwrap_or_default())
    };
    let origin = git.remote_url("origin")?.map(|u| owner_repo(&u)).unwrap_or_default();
    let who = Path::new(if origin.is_empty() { source.as_os_str() } else { origin.as_ref() })
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // sed reads lines split on newlines only: a carriage return stays part of its line
    let generic = String::from_utf8_lossy(&at(version, "generic")?).into_owned();
    let header = |key: &str| -> Option<String> {
        let prefix = format!("#! {key} = ");
        raw_lines(&generic).find_map(|l| l.strip_prefix(prefix.as_str()).map(str::to_string))
    };
    let pin_name =
        header("pin").filter(|s| !s.is_empty()).unwrap_or_else(|| "idealkb.toml".to_string());
    let mark = header("marker").filter(|s| !s.is_empty()).unwrap_or_else(|| "idealkb".to_string());
    if mark.contains(|c: char| ".*^$[]\\+?(){}|".contains(c)) {
        return Ok(refused(String::new(), format!("follow: the marker {mark:?} is a regular expression to the shell; rhizoid takes a plain word"), 2));
    }
    let from = header("from").unwrap_or_default();
    let pin = base.join(&pin_name);
    let pin_text = std::fs::read(&pin).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
    let old = pin_text
        .as_deref()
        .map(|t| captured(t, "version = \"", "\"").join("\n"))
        .unwrap_or_default();
    let is_own = |f: &str| pin_text.as_deref().is_some_and(|t| reads_as_own(t, f));

    let begin = format!("{mark}:begin");
    let io = |e: std::io::Error, p: &Path| PortError(format!("{}: {e}", p.display()));
    let mut said = String::new();
    let mut complained = String::new();
    let mut drift = false;
    // grep -vE '^[[:space:]]*(#|$)', then the shell's word split on space, tab and newline
    let files: Vec<String> = raw_lines(&generic)
        .filter(|l| {
            let t = l.trim_start_matches(|c: char| c.is_ascii_whitespace());
            !(t.is_empty() || t.starts_with('#'))
        })
        .flat_map(|l| {
            l.split([' ', '\t']).filter(|w| !w.is_empty()).map(str::to_string).collect::<Vec<_>>()
        })
        .collect();
    for f in files {
        let here = base.join(&f);
        let src_path = format!("{from}{f}");
        if is_own(&f) {
            if !old.is_empty() && !same(&at(&old, &src_path)?, &at(version, &src_path)?) {
                said.push_str(&format!("{f}: the base's own; {who} {version} changed it - look at whether the base takes it:\n"));
                // the shell writes the release's file before it diffs: one the release lacks ends it there
                let Some(new) = git.show(&tag, &src_path)? else {
                    return Ok(Followed { said, complained, code: 128 });
                };
                let (out, err) = diff_u(
                    cmd,
                    &here,
                    &new,
                    &format!("the base's {f}"),
                    &format!("{who} {version}'s"),
                )?;
                for line in raw_lines(&out) {
                    said.push_str(&format!("    {line}\n"));
                }
                complained.push_str(&err);
            } else {
                said.push_str(&format!("{f}: the base's own\n"));
            }
            continue;
        }
        // a file the list names that the release lacks ends the shell there (set -e, git show's 128): what was
        // laid stays, the pin is not moved
        let Some(new) = git.show(&tag, &src_path)? else {
            return Ok(Followed { said, complained, code: 128 });
        };
        if let Some(dir) = here.parent() {
            std::fs::create_dir_all(dir).map_err(|e| io(e, dir))?;
        }
        let had_file = std::fs::read(&here).ok();
        if contains(&new, &begin) {
            let part = marked(&new, &mark);
            match &had_file {
                Some(had) if contains(had, &begin) => {
                    let was = if old.is_empty() {
                        part.clone()
                    } else {
                        marked(&at(&old, &src_path)?, &mark)
                    };
                    if marked(had, &mark) != was {
                        drift = true;
                        said.push_str(&format!("{f}: the base changed its generic part - replaced; the diff shows the change to send upstream\n"));
                    }
                    let out = replace_part(had, &part, &mark);
                    if &out != had {
                        std::fs::write(&here, &out).map_err(|e| io(e, &here))?;
                        said.push_str(&format!("{f}: its generic part is {who} {version}'s\n"));
                    }
                }
                Some(had) => {
                    let mut out = had.clone();
                    out.push(b'\n');
                    out.extend_from_slice(&part);
                    std::fs::write(&here, &out).map_err(|e| io(e, &here))?;
                    said.push_str(&format!(
                        "{f}: {who} {version}'s generic part added at its end\n"
                    ));
                }
                None => {
                    std::fs::write(&here, &part).map_err(|e| io(e, &here))?;
                    said.push_str(&format!("{f}: made, from {who} {version}'s generic part\n"));
                }
            }
        } else {
            if let Some(had) = &had_file {
                if had != &new && (old.is_empty() || !same(&at(&old, &src_path)?, had)) {
                    drift = true;
                    said.push_str(&format!("{f}: the base had its own version - replaced; the diff shows it (send it upstream, or name it own)\n"));
                }
            }
            if had_file.as_ref() != Some(&new) {
                std::fs::write(&here, &new).map_err(|e| io(e, &here))?;
                said.push_str(&format!("{f}: {who} {version}'s\n"));
            }
        }
    }

    let source_name = if origin.is_empty() { who.clone() } else { origin.clone() };
    // the pin as it stands now: a generic file the release laid may be the pin itself
    let moved: Vec<u8> = match std::fs::read(&pin).ok() {
        Some(bytes) => move_version(&bytes, version),
        None => format!(
            "# This base follows {source_name} at the release named here: the files its generic list names, and the part of\n\
             # AGENTS.md and tables.toml between its marker lines. The pin job (ideal-knowledge-base bin/update-pins.sh) moves it;\n\
             # own names the files this base keeps as its own; source is where the job finds its releases.\n\
             source = \"{source_name}\"\nversion = \"{version}\"\nown = []\n"
        )
        .into_bytes(),
    };
    std::fs::write(&pin, &moved).map_err(|e| io(e, &pin))?;
    let want = format!("version = \"{version}\"");
    if !moved.split(|c| *c == b'\n').any(|l| l == want.as_bytes()) {
        return Ok(Followed {
            said,
            complained: complained + "follow: the pin did not move\n",
            code: 1,
        });
    }
    let before = if old.is_empty() { "not followed before" } else { old.as_str() };
    said.push_str(&format!("{who} {version}: {before} -> {version}\n"));
    Ok(Followed { said, complained, code: if drift { 3 } else { 0 } })
}

/// `owner/repo` from a remote URL, as the shell's sed reads it (`s/\.git$//; s|.*[:/]\([^:/]*/[^:/]*\)$|\1|`): the
/// text after the last `:` or `/` from which exactly one `/` and no `:` follow; the URL as it is when none does.
fn owner_repo(url: &str) -> String {
    let url = url.strip_suffix(".git").unwrap_or(url);
    let fits = |rest: &str| !rest.contains(':') && rest.matches('/').count() == 1;
    url.char_indices()
        .rev()
        .filter(|(_, c)| *c == ':' || *c == '/')
        .map(|(i, _)| &url[i + 1..])
        .find(|rest| fits(rest))
        .unwrap_or(url)
        .to_string()
}

/// The middle of each line that starts with `before` and ends with `after` (sed `s/^before\(.*\)after$/\1/p`).
/// Whether follow reads `file` as one of the base's own in its pin file `pin_text`, as the shell does: `case " $own "
/// in *" $f "*`, with `own` the one-line `own = [...]` lists, their quotes and spaces dropped, commas made spaces
/// and the lines joined by newlines - an entry is own only between spaces. Status asks it before it says follow
/// leaves a file whole.
pub fn reads_as_own(pin_text: &str, file: &str) -> bool {
    let own: String = captured(pin_text, "own = [", "]")
        .join("\n")
        .chars()
        .filter(|c| *c != '"' && *c != ' ')
        .collect::<String>()
        .replace(',', " ");
    format!(" {own} ").contains(&format!(" {file} "))
}

fn captured(text: &str, before: &str, after: &str) -> Vec<String> {
    raw_lines(text)
        .filter_map(|l| {
            let rest = l.strip_prefix(before)?;
            (rest.len() >= after.len()).then_some(())?;
            rest.strip_suffix(after).map(str::to_string)
        })
        .collect()
}

/// Two texts the way a shell compares `"$(...)"`: their NUL bytes and trailing newlines dropped.
fn same(a: &[u8], b: &[u8]) -> bool {
    let (a, b): (Vec<u8>, Vec<u8>) = (
        a.iter().copied().filter(|c| *c != 0).collect(),
        b.iter().copied().filter(|c| *c != 0).collect(),
    );
    let (a, b) = (a.as_slice(), b.as_slice());
    let trim =
        |x: &[u8]| -> usize { x.len() - x.iter().rev().take_while(|c| **c == b'\n').count() };
    a[..trim(a)] == b[..trim(b)]
}

fn contains(text: &[u8], needle: &str) -> bool {
    text.windows(needle.len().max(1)).any(|w| w == needle.as_bytes())
}

/// The lines from a `MARK:begin` line through its `MARK:end` line, each block, each line ended by a newline (awk).
fn marked(text: &[u8], mark: &str) -> Vec<u8> {
    let (begin, end) = (format!("{mark}:begin"), format!("{mark}:end"));
    let mut on = false;
    let mut out = Vec::new();
    for line in lines(text) {
        if contains(line, &begin) {
            on = true;
        }
        if on {
            out.extend_from_slice(line);
            out.push(b'\n');
        }
        if contains(line, &end) {
            on = false;
        }
    }
    out
}

/// `had` with its marked part replaced by `part` (the shell's awk): the part's lines at the first begin line, the
/// lines through each end line left out, every line ended by a newline.
fn replace_part(had: &[u8], part: &[u8], mark: &str) -> Vec<u8> {
    let (begin, end) = (format!("{mark}:begin"), format!("{mark}:end"));
    let mut part_lines = Some(lines(part).map(|l| l.to_vec()).collect::<Vec<_>>());
    let mut skip = false;
    let mut out = Vec::new();
    for line in lines(had) {
        if contains(line, &begin) {
            // awk's getline reads the part file once: a second begin line gets nothing
            for l in part_lines.take().unwrap_or_default() {
                out.extend_from_slice(&l);
                out.push(b'\n');
            }
            skip = true;
            continue;
        }
        if contains(line, &end) {
            skip = false;
            continue;
        }
        if !skip {
            out.extend_from_slice(line);
            out.push(b'\n');
        }
    }
    out
}

/// A text's lines as awk reads them: split on newlines, a last line without one still a line.
fn lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    let body = text.strip_suffix(b"\n").unwrap_or(text);
    let mut parts = body.split(|c| *c == b'\n');
    let empty = text.is_empty();
    std::iter::from_fn(move || if empty { None } else { parts.next() })
}

/// The version lines of a pin file moved to `version` (sed `s/^version = ".*"$/version = "V"/`).
fn move_version(text: &[u8], version: &str) -> Vec<u8> {
    let want = format!("version = \"{version}\"");
    let mut out = Vec::with_capacity(text.len());
    for line in text.split_inclusive(|c| *c == b'\n') {
        let (body, end): (&[u8], &[u8]) = match line.strip_suffix(b"\n") {
            Some(b) => (b, b"\n"),
            None => (line, b""),
        };
        let is_version =
            body.starts_with(b"version = \"") && body.ends_with(b"\"") && body.len() >= 12;
        out.extend_from_slice(if is_version { want.as_bytes() } else { body });
        out.extend_from_slice(end);
    }
    out
}

/// A text's lines as sed reads them: split on newlines only (a carriage return stays in its line), no empty last
/// line after a final newline.
fn raw_lines(text: &str) -> impl Iterator<Item = &str> {
    let body = text.strip_suffix('\n').unwrap_or(text);
    let empty = text.is_empty();
    body.split('\n').filter(move |_| !empty)
}

/// `diff -u --label A --label B FILE NEW`, as the shell runs it: its stdout and its stderr.
fn diff_u(
    cmd: &impl CommandPort,
    file: &Path,
    new: &[u8],
    label_a: &str,
    label_b: &str,
) -> Result<(String, String), PortError> {
    let scratch = std::env::temp_dir().join(format!(
        "rhizoid-follow-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&scratch, new).map_err(|e| PortError(format!("{}: {e}", scratch.display())))?;
    let argv: Vec<String> = [
        "diff",
        "-u",
        "--label",
        label_a,
        "--label",
        label_b,
        &file.display().to_string(),
        &scratch.display().to_string(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let dir = file.parent().unwrap_or(Path::new("/"));
    let dir = if dir.is_dir() { dir.to_path_buf() } else { std::env::temp_dir() };
    let run = cmd.run(&argv, &dir);
    let _ = std::fs::remove_file(&scratch);
    let (_, out, err) = run?;
    Ok((out, err))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shells_ways_are_kept() {
        assert!(same(b"a\n\n", b"a"));
        assert!(!same(b"a\nb", b"a"));
        assert_eq!(
            owner_repo("https://github.com/grenudi/ideal-knowledge-base.git"),
            "grenudi/ideal-knowledge-base"
        );
        assert_eq!(owner_repo("git@github.com:a/b.git"), "a/b");
        assert_eq!(captured("version = \"0.1.0\"\nx\n", "version = \"", "\""), ["0.1.0"]);
        assert_eq!(
            move_version(b"a\nversion = \"0.1.0\"\nb", "0.2.0"),
            b"a\nversion = \"0.2.0\"\nb"
        );
        assert_eq!(owner_repo("https://h/a/b/"), "b/");
        assert_eq!(owner_repo("https://a"), "/a");
        assert!(same(b"a\0b\n", b"ab"));
    }

    #[test]
    fn a_marked_part_is_found_and_replaced_as_awk_does() {
        let text = b"top\n<!-- m:begin -->\nv1\n<!-- m:end -->\nbottom";
        assert_eq!(marked(text, "m"), b"<!-- m:begin -->\nv1\n<!-- m:end -->\n");
        let part = b"<!-- m:begin -->\nv2\n<!-- m:end -->\n";
        assert_eq!(
            replace_part(text, part, "m"),
            b"top\n<!-- m:begin -->\nv2\n<!-- m:end -->\nbottom\n"
        );
    }
}
