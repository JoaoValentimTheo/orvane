//! Regression tests for output failures in the CLI (SPEC §0.2 rule 5).
//!
//! `orv tokens` must not panic when stdout cannot be written. There are two
//! distinct ways to make that happen, and only one of them is deterministic:
//!
//! * **A closed pipe.** `Stdio::piped()` plus dropping the read end is racy: the
//!   child may finish writing into the pipe buffer before the reader goes away,
//!   in which case it legitimately exits `0`. This test therefore accepts `0` or
//!   `2` and only rejects a panic (`101`) or a signal. It is still useful as a
//!   smoke test, but it is not a proof that the failure path was taken.
//! * **A full device.** On Linux, redirecting stdout to `/dev/full` makes every
//!   write fail with `ENOSPC`, deterministically. That test *does* prove the
//!   failure path, so it runs under `#[cfg(target_os = "linux")]`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Environment variable that points at the `orv` binary under test.
const ORV_BIN_ENV: &str = "ORV_BIN";

/// A lexer input that succeeds, so the only possible failure is the write.
const CASE: &str = "tests/golden/tokens_keywords.orv";

/// Exit code for "environment or usage error" (SPEC §10).
const EXIT_USAGE: i32 = 2;

/// Exit code Rust uses when a task unwinds out of `main`.
const EXIT_PANIC: i32 = 101;

#[test]
fn tokens_with_closed_stdout_does_not_panic() {
    // `Stdio::piped()` gives the child a pipe we then drop without reading.
    // Whether the child observes `EPIPE` depends on scheduling, so both `0`
    // (it finished before the reader left) and `2` (it caught the failure) are
    // correct; a panic or a signal never is.
    let mut child = match Command::new(orv_binary())
        .arg("tokens")
        .arg(CASE)
        .current_dir(workspace_root())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => panic!("cannot spawn the orv binary: {err}"),
    };

    // Drop the read end as soon as the child exists.
    drop(child.stdout.take());

    let status = match child.wait() {
        Ok(status) => status,
        Err(err) => panic!("cannot wait for the orv binary: {err}"),
    };

    let code = status.code();
    assert_ne!(
        code,
        Some(EXIT_PANIC),
        "the CLI panicked on a closed stdout: {status:?}"
    );
    assert!(
        code.is_some(),
        "the CLI was killed by a signal instead of exiting cleanly: {status:?}"
    );
    assert!(
        matches!(code, Some(0) | Some(EXIT_USAGE)),
        "a closed stdout must end in 0 (finished first) or 2 (write failed), got {status:?}"
    );
}

/// Deterministic proof that a failed stdout write becomes exit 2.
///
/// Linux-only: `/dev/full` is a character device whose writes always fail with
/// `ENOSPC`. Other platforms have no portable equivalent, so on macOS/Windows
/// this behaviour is covered only by the pipe test above.
#[cfg(target_os = "linux")]
#[test]
fn tokens_with_full_stdout_reports_usage_error() {
    let full = match std::fs::OpenOptions::new().write(true).open("/dev/full") {
        Ok(file) => file,
        // No `/dev/full` (unusual container): skip rather than fail.
        Err(_) => return,
    };

    let output = match Command::new(orv_binary())
        .arg("tokens")
        .arg(CASE)
        .current_dir(workspace_root())
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("cannot run the orv binary: {err}"),
    };

    assert_eq!(
        output.status.code(),
        Some(EXIT_USAGE),
        "a failed stdout write must be an environment error, got {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "stderr must not contain a panic: {stderr:?}"
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

    assert_eq!(output.status.code(), Some(EXIT_USAGE));
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
