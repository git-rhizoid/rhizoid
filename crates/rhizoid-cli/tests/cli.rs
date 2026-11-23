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
    let out = Command::new(bin()).args(["status", "--bogus"]).current_dir(&dir).output().unwrap();
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
