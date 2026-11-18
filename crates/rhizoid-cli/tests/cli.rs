//! Integration tests: run the real built binary, in a real temp directory,
//! and check its actual stdout/stderr/exit code - not just the library
//! functions underneath it. Mirrors argenv-cli's own `tests/cli.rs`.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rhizoid")
}

fn tempdir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhizoid-cli-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn init_writes_a_manifest_with_the_schema_directive() {
    let dir = tempdir();
    let out = Command::new(bin()).arg("init").current_dir(&dir).output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = std::fs::read_to_string(dir.join("rhizoid.toml")).unwrap();
    assert!(text.starts_with("#:schema "));
    assert!(text.contains("[defaults]"));
}

#[test]
fn init_twice_without_force_fails_and_does_not_overwrite() {
    let dir = tempdir();
    Command::new(bin()).args(["init", "--org", "first-org"]).current_dir(&dir).output().unwrap();
    let out = Command::new(bin()).arg("init").current_dir(&dir).output().unwrap();
    assert!(!out.status.success());
    let text = std::fs::read_to_string(dir.join("rhizoid.toml")).unwrap();
    assert!(text.contains("first-org"), "existing manifest must be untouched");
}

#[test]
fn init_with_force_overwrites() {
    let dir = tempdir();
    Command::new(bin()).args(["init", "--org", "first-org"]).current_dir(&dir).output().unwrap();
    let out = Command::new(bin())
        .args(["init", "--org", "second-org", "--force"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = std::fs::read_to_string(dir.join("rhizoid.toml")).unwrap();
    assert!(text.contains("second-org"));
}

#[test]
fn add_without_a_positional_fails_before_touching_anything() {
    let dir = tempdir();
    let out = Command::new(bin()).arg("add").current_dir(&dir).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("missing <source>"));
}

#[test]
fn add_reports_not_implemented_rather_than_silently_succeeding() {
    let dir = tempdir();
    let out = Command::new(bin())
        .args(["add", "octocat/hello-world"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not implemented yet"));
}

#[test]
fn remove_reports_a_clear_error_for_an_unknown_module() {
    let dir = tempdir();
    Command::new(bin()).arg("init").current_dir(&dir).output().unwrap();
    let out =
        Command::new(bin()).args(["remove", "does-not-exist"]).current_dir(&dir).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no module named"));
}

#[test]
fn unknown_command_exits_with_status_2_and_names_it() {
    let out = Command::new(bin()).arg("bogus-command").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("bogus-command"));
}

#[test]
fn version_flag_prints_the_crate_version() {
    let out = Command::new(bin()).arg("--version").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn a_bare_rhizoid_prints_its_usage_and_ends_0() {
    let out = Command::new(bin()).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Usage: rhizoid"));
}

#[test]
fn the_short_version_flag_works_as_it_did() {
    let out = Command::new(bin()).arg("-V").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn a_flag_it_does_not_know_ends_2_and_writes_nothing() {
    let dir = tempdir();
    let out = Command::new(bin()).args(["init", "--bogus"]).current_dir(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!dir.join("rhizoid.toml").exists());
}

#[test]
fn a_report_to_a_closed_pipe_is_not_a_panic() {
    use std::process::Stdio;
    let mut child = Command::new(bin()).arg("man").stdout(Stdio::piped()).spawn().unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_ne!(out.status.code(), Some(101), "{:?}", out.status);
}
