//! `rhizoid follow` against the shell it stands in for, ideal-knowledge-base's `bin/follow.sh`: each scenario is
//! built twice, the shell lays it into one copy and rhizoid into the other, and what each printed, how each ended
//! and the trees they left are compared byte for byte; then idealkb's own `bin/test-follow.sh` runs with its
//! follow.sh handing over to rhizoid. It needs a clone of ideal-knowledge-base: RHIZOID_IDEALKB=DIR (unset, the
//! scenarios that need no shell still run against what follow.sh is known to do).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir(what: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-follow-{what}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "tag.gpgSign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let p = root.join(path);
        if *text == DELETED {
            let _ = std::fs::remove_file(&p);
            continue;
        }
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}

/// A file's content that says: this release deletes it.
const DELETED: &str = "\u{0}deleted";

/// A source repository: each release its files, tagged vVERSION.
fn source(root: &Path, releases: &[(&str, &[(&str, &str)])]) -> PathBuf {
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    for (version, files) in releases {
        write(&src, files);
        git(&src, &["add", "-A"]);
        git(&src, &["commit", "-q", "--allow-empty", "-m", version]);
        git(&src, &["tag", &format!("v{version}")]);
    }
    src
}

/// Every file under `dir`, by path, with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(
                    p.strip_prefix(dir).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

fn rhizoid(src: &Path, base: &Path, version: &str) -> Output {
    Command::new(bin())
        .args(["follow", "--source"])
        .arg(src)
        .arg(base)
        .arg(version)
        .output()
        .unwrap()
}

/// One scenario: a source, a base as it stands, a release to lay.
struct Scenario {
    name: &'static str,
    origin: Option<&'static str>,
    releases: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
    base: Vec<(&'static str, &'static str)>,
    version: &'static str,
}

fn scenarios() -> Vec<Scenario> {
    let generic = "#! pin = devicebase.toml\n#! marker = template\n# a second source\nqueries/phone.sql\nrecords/part.sql\nAGENTS.md\n";
    let agents = |v: &'static str| -> &'static str {
        match v {
            "1" => "top\n<!-- template:begin -->\nthe template says v1\n<!-- template:end -->\nbottom\n",
            _ => "top\n<!-- template:begin -->\nthe template says v2\n<!-- template:end -->\nbottom\n",
        }
    };
    let two = |extra2: Vec<(&'static str, &'static str)>| {
        let mut v2 = vec![("queries/phone.sql", "-- phone v2\n"), ("AGENTS.md", agents("2"))];
        v2.extend(extra2);
        vec![
            (
                "0.1.0",
                vec![
                    ("generic", generic),
                    ("queries/phone.sql", "-- phone v1\n"),
                    ("records/part.sql", "-- part v1\n"),
                    ("AGENTS.md", agents("1")),
                ],
            ),
            ("0.2.0", v2),
        ]
    };
    let pin = "source = \"x/src\"\nversion = \"0.1.0\"\nown = [\"records/part.sql\"]\n";
    vec![
        Scenario {
            origin: None,
            name: "test-follow's own: a second source, its marker part, an own file",
            releases: two(vec![]),
            base: vec![
                ("AGENTS.md", "own\n<!-- template:begin -->\nthe template says v1\n<!-- template:end -->\nmine\n"),
                ("records/part.sql", "-- the base's own part\n"),
                ("devicebase.toml", pin),
            ],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "the base changed a whole generic file and its marked part: both said, exit 3",
            releases: two(vec![]),
            base: vec![
                ("AGENTS.md", "own\n<!-- template:begin -->\nthe base's own words\n<!-- template:end -->\nmine\n"),
                ("queries/phone.sql", "-- phone, changed by the base\n"),
                ("devicebase.toml", pin),
            ],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "an own file the release changed: its diff shown",
            releases: two(vec![("records/part.sql", "-- part v2\n")]),
            base: vec![("records/part.sql", "-- the base's own part\n"), ("devicebase.toml", pin)],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "a first follow: the pin made, a part added to a file without markers, a file made",
            releases: two(vec![]),
            base: vec![("AGENTS.md", "the base's own AGENTS, no markers")],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "a folder slice, laid at the root; a listed file the release lacks ends it, as set -e does",
            releases: vec![(
                "1.0.0",
                vec![
                    ("generic", "#! pin = layer.toml\n#! from = layer/\nqueries/slice.sql\nbin/tool.sh missing.txt\n"),
                    ("layer/queries/slice.sql", "-- slice v1\n"),
                    ("layer/bin/tool.sh", "tool v1"),
                    ("queries/slice.sql", "-- not in the slice\n"),
                ],
            )],
            base: vec![],
            version: "1.0.0",
        },
        Scenario {
            origin: Some("https://github.com/grenudi/ideal-knowledge-base.git"),
            name: "a source with an origin: its name said, and the pin made names it",
            releases: two(vec![]),
            base: vec![],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "an own file the release deleted: the shell stops after its header",
            releases: two(vec![("records/part.sql", DELETED)]),
            base: vec![("records/part.sql", "-- the base's own part\n"), ("devicebase.toml", pin)],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "the pin is a generic file: laid, then moved",
            releases: vec![("0.1.0", vec![("generic", "#! pin = devicebase.toml\ndevicebase.toml\nqueries/phone.sql\n"), ("devicebase.toml", "# laid by the source\nsource = \"y/src\"\nversion = \"9\"\n"), ("queries/phone.sql", "-- phone\n")])],
            base: vec![("devicebase.toml", "source = \"x/src\"\nversion = \"0.0.1\"\n")],
            version: "0.1.0",
        },
        Scenario {
            origin: None,
            name: "a CRLF pin: sed sees no version nor own",
            releases: two(vec![]),
            base: vec![("records/part.sql", "-- the base's own part\n"), ("devicebase.toml", "source = \"x/src\"\r\nversion = \"0.1.0\"\r\nown = [\"records/part.sql\"]\r\n")],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "a CRLF generic: a name with its carriage return",
            releases: vec![("0.1.0", vec![("generic", "#! pin = p.toml\r\nA.md\r\n"), ("A.md", "a\n")])],
            base: vec![],
            version: "0.1.0",
        },
        Scenario {
            origin: None,
            name: "own on two lines and with a tab: only entries between spaces are own",
            releases: two(vec![("records/part.sql", "-- part v2\n")]),
            base: vec![("records/part.sql", "-- the base's own part\n"), ("queries/phone.sql", "-- the base's phone\n"), ("devicebase.toml", "version = \"0.1.0\"\nown = [\"records/part.sql\"]\nown = [\"queries/phone.sql\",\t\"AGENTS.md\"]\n")],
            version: "0.2.0",
        },
        Scenario {
            origin: Some("https://h/a/b/"),
            name: "a remote with a trailing slash names the pin it makes as sed reads it",
            releases: two(vec![]),
            base: vec![],
            version: "0.2.0",
        },
        Scenario {
            origin: None,
            name: "a release that does not exist",
            releases: two(vec![]),
            base: vec![("devicebase.toml", pin)],
            version: "9.9.9",
        },
    ]
}

/// Build a scenario: its source, and its base twice (one for the shell, one for rhizoid).
fn build(s: &Scenario) -> (PathBuf, PathBuf, PathBuf) {
    let root = tempdir("scenario");
    let releases: Vec<(&str, &[(&str, &str)])> =
        s.releases.iter().map(|(v, f)| (*v, f.as_slice())).collect();
    let src = source(&root, &releases);
    if let Some(url) = s.origin {
        git(&src, &["remote", "add", "origin", url]);
    }
    let (a, b) = (root.join("base-shell"), root.join("base-rhizoid"));
    for base in [&a, &b] {
        std::fs::create_dir_all(base).unwrap();
        write(base, &s.base);
    }
    (src, a, b)
}

#[test]
fn rhizoid_follow_lays_and_says_what_follow_sh_does_byte_for_byte() {
    let Some(kb) = std::env::var_os("RHIZOID_IDEALKB").map(PathBuf::from) else {
        eprintln!("RHIZOID_IDEALKB is not set: the comparison with follow.sh is not run");
        return;
    };
    let follow_sh = kb.join("bin/follow.sh");
    assert!(follow_sh.is_file(), "no {}", follow_sh.display());
    for s in scenarios() {
        let (src, a, b) = build(&s);
        let shell = Command::new("sh")
            .arg(&follow_sh)
            .arg("--source")
            .arg(&src)
            .arg(&a)
            .arg(s.version)
            .output()
            .unwrap();
        let ours = rhizoid(&src, &b, s.version);
        let rename = |o: &Output, from: &Path, to: &Path| {
            String::from_utf8_lossy(&o.stderr)
                .replace(&from.display().to_string(), &to.display().to_string())
        };
        assert_eq!(
            String::from_utf8_lossy(&ours.stdout),
            String::from_utf8_lossy(&shell.stdout),
            "{}: what was said",
            s.name
        );
        assert_eq!(ours.status.code(), shell.status.code(), "{}: how it ended", s.name);
        assert_eq!(
            rename(&ours, &b, &a),
            rename(&shell, &a, &a),
            "{}: what was complained",
            s.name
        );
        assert_eq!(tree(&b), tree(&a), "{}: the trees left", s.name);
    }
}

#[test]
fn idealkb_test_follow_passes_with_rhizoid_standing_in() {
    let Some(kb) = std::env::var_os("RHIZOID_IDEALKB").map(PathBuf::from) else {
        eprintln!("RHIZOID_IDEALKB is not set: test-follow.sh is not run");
        return;
    };
    let root = tempdir("test-follow");
    let copy = root.join("kb");
    let ok = Command::new("cp").arg("-a").arg(&kb).arg(&copy).status().unwrap();
    assert!(ok.success(), "copy {}", kb.display());
    // follow.sh hands over to rhizoid, its source the clone it is in unless --source names one
    let shim = format!(
        "#!/bin/sh\nset -eu\nkb=$(cd \"$(dirname \"$0\")/..\" && pwd)\nif [ \"${{1:-}}\" = --source ]; then exec {bin} follow \"$@\"; fi\nexec {bin} follow --source \"$kb\" \"$@\"\n",
        bin = bin()
    );
    std::fs::write(copy.join("bin/follow.sh"), shim).unwrap();
    let out = Command::new("sh").arg(copy.join("bin/test-follow.sh")).output().unwrap();
    assert!(
        out.status.success(),
        "test-follow.sh with rhizoid: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhizoid_follow_does_what_follow_sh_is_known_to_do() {
    let s = &scenarios()[0];
    let (src, _, b) = build(s);
    let out = rhizoid(&src, &b, s.version);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "queries/phone.sql: src 0.2.0's\nrecords/part.sql: the base's own\nAGENTS.md: its generic part is src 0.2.0's\nsrc 0.2.0: 0.1.0 -> 0.2.0\n"
    );
    let t = tree(&b);
    assert_eq!(
        t["AGENTS.md"],
        b"own\n<!-- template:begin -->\nthe template says v2\n<!-- template:end -->\nmine\n"
    );
    assert_eq!(t["records/part.sql"], b"-- the base's own part\n");
    assert_eq!(
        t["devicebase.toml"],
        b"source = \"x/src\"\nversion = \"0.2.0\"\nown = [\"records/part.sql\"]\n"
    );
    let missing = rhizoid(&src, &b, "9.9.9");
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("follow: no release v9.9.9 in"));
}

#[test]
fn a_source_named_through_a_link_keeps_the_links_name_as_the_shell_does() {
    let Some(kb) = std::env::var_os("RHIZOID_IDEALKB").map(PathBuf::from) else { return };
    let s = &scenarios()[3];
    let (src, a, b) = build(s);
    let link = src.parent().unwrap().join("linkname");
    std::os::unix::fs::symlink(&src, &link).unwrap();
    let shell = Command::new("sh")
        .arg(kb.join("bin/follow.sh"))
        .arg("--source")
        .arg(&link)
        .arg(&a)
        .arg(s.version)
        .output()
        .unwrap();
    let ours = rhizoid(&link, &b, s.version);
    assert_eq!(String::from_utf8_lossy(&ours.stdout), String::from_utf8_lossy(&shell.stdout));
    assert_eq!(tree(&b), tree(&a));
}

#[test]
fn anything_after_the_version_is_refused() {
    let s = &scenarios()[0];
    let (src, _, b) = build(s);
    let out = Command::new(bin())
        .args(["follow", "--source"])
        .arg(&src)
        .arg(&b)
        .args([s.version, "extra"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        tree(&b)
            .get("devicebase.toml")
            .is_some_and(|p| p.ends_with(b"version = \"0.1.0\"\nown = [\"records/part.sql\"]\n")),
        "nothing laid"
    );
}
