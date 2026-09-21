//! Regression tests for output failures in the CLI (SPEC §0.2 rule 5).
//!
//! `orv tokens` must not panic when stdout cannot be written — for example when
//! the reader closes the pipe (`orv tokens x.orv | head`). The write failure is
//! an environment problem, so it maps to exit code `2`, not a crash.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Environment variable that points at the `orv` binary under test.
const ORV_BIN_ENV: &str = "ORV_BIN";

#[test]
fn tokens_with_closed_stdout_does_not_panic() {
    let case = "tests/golden/tokens_keywords.orv";

    // `Stdio::piped()` gives the child a pipe we then drop without reading:
    // once the pipe buffer is full (or immediately, on an empty pipe), the
    // child's writes fail with `EPIPE`.
    let mut child = match Command::new(orv_binary())
        .arg("tokens")
        .arg(case)
        .current_dir(workspace_root())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => panic!("cannot spawn the orv binary: {err}"),
    };

    // Drop the read end before the child writes anything.
    drop(child.stdout.take());

    let status = match child.wait() {
        Ok(status) => status,
        Err(err) => panic!("cannot wait for the orv binary: {err}"),
    };

    // A closed pipe is an environment error: exit 2. What must never happen is
    // a panic, which would show up as 101 or as a signal.
    let code = status.code();
    assert_ne!(
        code,
        Some(101),
        "the CLI panicked on a closed stdout (exit 101)"
    );
    assert!(
        status.code().is_some(),
        "the CLI was killed by a signal instead of exiting cleanly: {status:?}"
    );
    assert_eq!(
        code,
        Some(2),
        "a closed stdout must be reported as an environment error (exit 2), got {status:?}"
    );
}

#[test]
fn tokens_on_a_missing_file_reports_usage_error() {
    let output = Command::new(orv_binary())
        .arg("tokens")
        .arg("tests/golden/definitely-missing.orv")
        .current_dir(workspace_root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("running the orv binary");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "nothing should reach stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot read"),
        "stderr should explain the failure, got: {stderr:?}"
    );
    assert!(
        !stderr.contains("panicked"),
        "stderr must not contain a panic: {stderr:?}"
    );
}

/// Resolves the binary under test, like the golden harness does.
fn orv_binary() -> PathBuf {
    if let Some(path) = std::env::var_os(ORV_BIN_ENV) {
        return PathBuf::from(path);
    }
    let mut dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let candidate = dir.join(format!("orv{}", std::env::consts::EXE_SUFFIX));
    if candidate.exists() {
        return candidate;
    }
    PathBuf::from("orv")
}

/// The workspace root, where golden paths in test headers are relative to.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn workspace_root_is_the_repo_root() {
    // Guards the other tests' assumptions about relative paths.
    assert!(
        workspace_root().join("Cargo.toml").is_file(),
        "workspace root {} has no Cargo.toml",
        workspace_root().display()
    );
}
