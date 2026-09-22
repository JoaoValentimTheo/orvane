//! End-to-end golden harness (SPEC §11 item 2, ADR 0003).
//!
//! It walks `tests/golden/` and, for every `<name>.orv`, runs the command
//! declared in the file's header and compares the result with the checked-in
//! expectation files:
//!
//! ```text
//! // orv: <subcommand> [args...]     <- header, first non-empty line
//! // exit: <code>                    <- optional, defaults to 0
//! ```
//!
//! `%f` in the command line expands to the golden file's path relative to the
//! workspace root, so a case can operate on its own source:
//! `// orv: tokens %f`.
//!
//! The harness is **strict** (ADR 0004):
//!
//! * `<name>.out` — expected **stdout**, compared byte for byte. When the file
//!   is absent, the expected stdout is the empty string, so a case that prints
//!   anything must ship a `.out`.
//! * `<name>.err` — expected **diagnostics**, compared against the normalized
//!   `CODE:linha:coluna: mensagem` lines from stderr (SPEC §8.1). When the file
//!   is absent, the case must produce **zero** diagnostics. The pretty `ariadne`
//!   drawing is deliberately ignored so the expectation stays stable.
//! * `// exit: N` — expected process exit status, default `0`.
//!
//! Snapshots are never updated automatically (SPEC §11 item 2): a mismatch is a
//! test failure, and fixing it is a deliberate edit.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Directory holding the golden files, relative to the workspace root.
const GOLDEN_DIR: &str = "tests/golden";

/// Environment variable that points at the `orv` binary under test.
const ORV_BIN_ENV: &str = "ORV_BIN";

/// A golden case's header: the command to run and the expected exit code.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Header {
    /// `argv[0]` is the `orv` subcommand.
    argv: Vec<String>,
    /// Expected process exit status; `0` unless `// exit: N` overrides it.
    exit: i32,
}

/// `orv version` must emit `LF` bytes on every OS (ADR 0002).
///
/// This asserts on the **raw** stdout bytes, before any normalization, so the
/// harness's CRLF folding cannot mask a `CR` produced by the binary itself.
/// If this fails on Windows, the fix belongs in the CLI, not here.
#[test]
fn version_stdout_contains_no_carriage_return() {
    let output = run_command(&orv_binary(), &["version".to_owned()]).expect("running orv version");

    let stdout = &output.stdout;
    assert!(
        !stdout.contains(&b'\r'),
        "orv version wrote a CR byte on {}: {:?}",
        std::env::consts::OS,
        stdout
    );
    assert!(
        stdout.ends_with(b"\n"),
        "orv version must end with a single LF, got {stdout:?}"
    );
    assert_eq!(
        stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "orv version must print exactly one line, got {stdout:?}"
    );
}

#[test]
fn golden_suite() {
    let golden_dir = workspace_root().join(GOLDEN_DIR);
    let orv = orv_binary();

    let mut cases: Vec<PathBuf> = Vec::new();
    collect_orv_files(&golden_dir, &mut cases);
    assert!(
        !cases.is_empty(),
        "no golden cases found under {}",
        golden_dir.display()
    );
    cases.sort();

    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        if let Err(report) = run_case(case, &orv) {
            failures.push(report);
        }
    }

    assert!(
        failures.is_empty(),
        "{} golden case(s) failed:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Runs one golden case and returns a human-readable report on failure.
fn run_case(orv_file: &Path, orv_bin: &Path) -> Result<(), String> {
    let name = orv_file
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("<unknown>");

    let source = std::fs::read_to_string(orv_file)
        .map_err(|err| format!("{name}: cannot read {}: {err}", orv_file.display()))?;

    let header = parse_header(&source).ok_or_else(|| {
        format!("{name}: missing `// orv: <subcommand>` header on the first non-empty line")
    })?;

    let argv = resolve_argv(&header.argv, orv_file);
    let output = run_command(orv_bin, &argv)
        .map_err(|err| format!("{name}: cannot run `{orv_bin:?} {}`: {err}", argv.join(" ")))?;

    let stem = orv_file.with_extension("");
    let mut problems: Vec<String> = Vec::new();

    check_exit_code(&header, &output, &stem, &mut problems);
    check_stdout(&output, &stem, &mut problems)?;
    check_diagnostics(&output, &stem, &mut problems)?;

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Compares the process exit code with the header's expectation.
fn check_exit_code(header: &Header, output: &Output, stem: &Path, problems: &mut Vec<String>) {
    let actual = output.status.code();
    if actual == Some(header.exit) {
        return;
    }
    problems.push(format!(
        "--- {} (exit status) ---\nexpected: {}\nactual: {}",
        file_label(stem),
        header.exit,
        actual.map_or_else(|| "<terminated by signal>".to_owned(), |c| c.to_string()),
    ));
}

/// Compares stdout, requiring an empty stdout when `.out` is absent.
fn check_stdout(output: &Output, stem: &Path, problems: &mut Vec<String>) -> Result<(), String> {
    let out_path = stem.with_extension("out");
    let expected = if out_path.exists() {
        Some(read_expectation(&out_path)?)
    } else {
        None
    };
    let actual = decode_output(&output.stdout);
    match compare_bytes("stdout", expected.as_deref(), &actual) {
        Ok(()) => Ok(()),
        Err(report) => {
            problems.push(report);
            Ok(())
        }
    }
}

/// Compares diagnostics, requiring zero diagnostics when `.err` is absent.
fn check_diagnostics(
    output: &Output,
    stem: &Path,
    problems: &mut Vec<String>,
) -> Result<(), String> {
    let err_path = stem.with_extension("err");
    let expected = if err_path.exists() {
        Some(read_expectation(&err_path)?)
    } else {
        None
    };
    let actual = normalize_diagnostics(&decode_output(&output.stderr));
    match compare_bytes("diagnostics", expected.as_deref(), &actual) {
        Ok(()) => Ok(()),
        Err(report) => {
            problems.push(report);
            Ok(())
        }
    }
}

/// Replaces the `%f` placeholder in the header with the golden file's path,
/// relative to the workspace root (the child's working directory).
///
/// A case that lexes or checks its own file writes `// orv: tokens %f`.
fn resolve_argv(argv: &[String], orv_file: &Path) -> Vec<String> {
    let root = workspace_root();
    let relative = orv_file.strip_prefix(&root).unwrap_or(orv_file);
    let path = relative.to_string_lossy().replace('\\', "/");
    argv.iter()
        .map(|arg| {
            if arg == "%f" {
                path.clone()
            } else {
                arg.clone()
            }
        })
        .collect()
}

/// Reads the header lines and extracts the command and the expected exit code.
///
/// The first non-empty line must be `// orv: <subcommand> [args...]`. A later
/// line may be `// exit: N`; `N` is the expected exit status and defaults to `0`.
/// Unknown `// key:` lines are ignored so the header can grow without breaking
/// older cases.
fn parse_header(source: &str) -> Option<Header> {
    let lines: Vec<&str> = source.lines().collect();
    let first = lines.iter().position(|l| !l.trim().is_empty())?;
    let argv = parse_command_line(lines.get(first)?)?;
    let exit = lines
        .iter()
        .skip(first + 1)
        .find_map(|line| parse_exit_line(line))
        .unwrap_or(0);
    Some(Header { argv, exit })
}

fn parse_command_line(line: &str) -> Option<Vec<String>> {
    let rest = line.trim().strip_prefix("//")?.trim();
    let rest = rest.strip_prefix("orv:")?;
    let argv: Vec<String> = rest.split_whitespace().map(str::to_owned).collect();
    if argv.is_empty() { None } else { Some(argv) }
}

fn parse_exit_line(line: &str) -> Option<i32> {
    let rest = line.trim().strip_prefix("//")?.trim();
    let rest = rest.strip_prefix("exit:")?.trim();
    rest.parse::<i32>().ok()
}

/// Compares one stream's text with its expectation.
///
/// `expected` is `None` when the expectation file is absent, in which case the
/// stream must be empty. This is the single implementation of the strict rule,
/// so `check_stdout`/`check_diagnostics` cannot drift apart.
fn compare_bytes(what: &str, expected: Option<&str>, actual: &str) -> Result<(), String> {
    let expected = expected.unwrap_or("");
    if actual == expected {
        Ok(())
    } else {
        Err(diff_report(what, expected, actual))
    }
}

fn read_expectation(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("cannot read {}: {err}", path.display()))
}

/// Decodes captured stdout/stderr to text with `LF` line endings.
///
/// Windows pipes produce `CRLF`, while the expectation files are checked in
/// with `LF`, so without this normalization the comparison would differ only by
/// line endings (see ADR 0003).
fn decode_output(bytes: &[u8]) -> String {
    normalize_line_endings(&String::from_utf8_lossy(bytes)).into_owned()
}

/// Replaces `CRLF` with `LF`, leaving lone `CR` bytes untouched.
fn normalize_line_endings(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("\r\n") {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("<unknown>")
        .to_owned()
}

fn diff_report(what: &str, expected: &str, actual: &str) -> String {
    format!(
        "--- {what} mismatch ---\nexpected:\n{}\nactual:\n{}\n---",
        indent(expected),
        indent(actual)
    )
}

fn indent(text: &str) -> String {
    if text.is_empty() {
        return "    <empty>".to_owned();
    }
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Keeps only the stable diagnostic lines, dropping `ariadne`'s decoration.
///
/// A diagnostic line starts at column 0 and looks like `CODE:line:col: message`
/// where `CODE` is `[EW]` + 4 digits or `R` + 4 digits (SPEC §8.2).
fn normalize_diagnostics(stderr: &str) -> String {
    let mut out = String::new();
    for line in stderr.lines() {
        if is_diagnostic_line(line) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn is_diagnostic_line(line: &str) -> bool {
    let Some((code, rest)) = line.split_once(':') else {
        return false;
    };
    if !is_diagnostic_code(code) {
        return false;
    }
    let mut parts = rest.splitn(2, ':');
    let line_no = parts.next().unwrap_or_default();
    let remainder = parts.next().unwrap_or_default();
    if line_no.parse::<u32>().is_err() {
        return false;
    }
    let col = remainder
        .split_once(':')
        .map(|(c, _)| c)
        .unwrap_or(remainder);
    col.parse::<u32>().is_ok()
}

fn is_diagnostic_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    match bytes {
        [b'E' | b'W' | b'R', d0, d1, d2, d3] => [d0, d1, d2, d3].iter().all(|b| b.is_ascii_digit()),
        _ => false,
    }
}

fn collect_orv_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_orv_files(&path, out);
        } else if path.extension() == Some(OsStr::new("orv")) {
            out.push(path);
        }
    }
}

/// Runs the `orv` binary with `argv[0]` as the subcommand.
///
/// The child runs with the **workspace root** as its working directory, because
/// header arguments are written relative to it (e.g. `tokens tests/golden/x.orv`)
/// while the test process itself starts in the crate directory.
fn run_command(orv_bin: &Path, argv: &[String]) -> io::Result<Output> {
    let Some((subcommand, args)) = argv.split_first() else {
        return Err(io::Error::other("empty command header"));
    };
    Command::new(orv_bin)
        .arg(subcommand)
        .args(args)
        .current_dir(workspace_root())
        .output()
}

/// Resolves the binary under test.
///
/// Uses `$ORV_BIN` when set (CI builds it explicitly); otherwise falls back to
/// the `orv` binary that cargo builds for this package, derived from the test
/// executable's own path so no `unwrap()` on environment variables is needed.
fn orv_binary() -> PathBuf {
    if let Some(path) = std::env::var_os(ORV_BIN_ENV) {
        return PathBuf::from(path);
    }
    let mut dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    dir.pop(); // deps/
    if dir.ends_with("deps") {
        dir.pop();
    }
    let candidate = dir.join(format!("orv{}", std::env::consts::EXE_SUFFIX));
    if candidate.exists() {
        return candidate;
    }
    PathBuf::from("orv")
}

fn workspace_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `<root>/crates/orv-cli`, so climbing two levels
    // reaches the workspace root where `tests/golden/` lives.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(test)]
mod harness_unit_tests {
    use super::*;

    fn header(argv: &[&str], exit: i32) -> Header {
        Header {
            argv: argv.iter().map(|s| (*s).to_owned()).collect(),
            exit,
        }
    }

    #[test]
    fn header_parsing_ignores_blank_lines() {
        let src = "\n\n// orv: version\norvane\n";
        assert_eq!(parse_header(src), Some(header(&["version"], 0)));
    }

    #[test]
    fn header_requires_the_marker() {
        assert_eq!(parse_header("// just a comment\n"), None);
        assert_eq!(parse_header("fn main() {}\n"), None);
    }

    #[test]
    fn header_supports_arguments() {
        let src = "// orv: run examples/fib.orv\n";
        assert_eq!(
            parse_header(src),
            Some(header(&["run", "examples/fib.orv"], 0))
        );
    }

    #[test]
    fn file_placeholder_expands_to_a_workspace_relative_path() {
        let golden = workspace_root().join("tests/golden/x.orv");
        let argv = vec!["tokens".to_owned(), "%f".to_owned()];
        assert_eq!(
            resolve_argv(&argv, &golden),
            vec!["tokens".to_owned(), "tests/golden/x.orv".to_owned()]
        );
    }

    #[test]
    fn file_placeholder_expands_in_any_position_and_leaves_other_args_alone() {
        let golden = workspace_root().join("tests/golden/x.orv");
        let argv = vec![
            "run".to_owned(),
            "--json".to_owned(),
            "%f".to_owned(),
            "extra".to_owned(),
        ];
        assert_eq!(
            resolve_argv(&argv, &golden),
            vec![
                "run".to_owned(),
                "--json".to_owned(),
                "tests/golden/x.orv".to_owned(),
                "extra".to_owned(),
            ]
        );
    }

    #[test]
    fn argv_without_placeholder_is_unchanged() {
        let golden = workspace_root().join("tests/golden/x.orv");
        let argv = vec!["version".to_owned()];
        assert_eq!(resolve_argv(&argv, &golden), argv);
    }

    #[test]
    fn header_defaults_exit_to_zero() {
        assert_eq!(
            parse_header("// orv: version\n"),
            Some(header(&["version"], 0))
        );
    }

    #[test]
    fn header_parses_exit_override() {
        let src = "// orv: check broken.orv\n// exit: 1\n";
        assert_eq!(parse_header(src), Some(header(&["check", "broken.orv"], 1)));
    }

    #[test]
    fn header_parses_signal_exit_code() {
        let src = "// orv: repl\n// exit: 130\n";
        assert_eq!(parse_header(src), Some(header(&["repl"], 130)));
    }

    #[test]
    fn header_ignores_unknown_and_malformed_directives() {
        // Unknown keys are ignored (forward compatible) ...
        let src = "// orv: version\n// something: else\n";
        assert_eq!(parse_header(src), Some(header(&["version"], 0)));

        // ... and a malformed `exit:` falls back to the default rather than
        // silently inventing a code.
        let src = "// orv: version\n// exit: soon\n";
        assert_eq!(parse_header(src), Some(header(&["version"], 0)));
    }

    #[test]
    fn header_requires_the_command_line_to_come_first() {
        // The `orv:` line is the header anchor; a directive before it is not a
        // valid header, so the case is rejected instead of guessing.
        let src = "// exit: 7\n// orv: version\n";
        assert_eq!(parse_header(src), None);
    }

    #[test]
    fn diagnostic_normalization_keeps_only_stable_lines() {
        let stderr = "Error: something\n  --> file.orv\nE0101:2:5: unexpected token\n   ^\nR0001:9:1: division by zero\n";
        assert_eq!(
            normalize_diagnostics(stderr),
            "E0101:2:5: unexpected token\nR0001:9:1: division by zero\n"
        );
    }

    #[test]
    fn diagnostic_line_detection_is_strict() {
        assert!(is_diagnostic_line("E0001:1:1: invalid character"));
        assert!(is_diagnostic_line("W0001:10:20: unused"));
        assert!(is_diagnostic_line("R0010:3:4: unsatisfied"));
        assert!(!is_diagnostic_line("E01:1:1: too short"));
        assert!(!is_diagnostic_line("Error:1:1: wrong prefix"));
        assert!(!is_diagnostic_line("E0001::1: missing line"));
        assert!(!is_diagnostic_line("E0001:1:1 no second colon in coords"));
    }

    #[test]
    fn empty_expectation_renders_placeholder() {
        assert_eq!(indent(""), "    <empty>");
        assert_eq!(indent("a\nb"), "    a\n    b");
    }

    #[test]
    fn decode_output_normalizes_crlf() {
        // Windows pipes deliver CRLF; expectation files are LF-only.
        assert_eq!(decode_output(b"orv 0.1.0\r\n"), "orv 0.1.0\n");
        assert_eq!(decode_output(b"a\r\nb\r\n"), "a\nb\n");
    }

    #[test]
    fn decode_output_keeps_lf_and_lone_cr() {
        assert_eq!(decode_output(b"a\nb\n"), "a\nb\n");
        assert_eq!(normalize_line_endings("a\rb"), "a\rb");
        assert_eq!(normalize_line_endings("plain"), "plain");
    }

    // --- Strictness rules, proved with comparisons that MUST fail -----------
    //
    // `check_stdout`/`check_diagnostics` work on a real `Output`, which cannot
    // be constructed portably, so the strict rules are proved through
    // `compare_bytes`: the same predicate the checks delegate to.

    fn strict_stdout(actual: &str, expected: Option<&str>) -> Result<(), String> {
        compare_bytes("stdout", expected, actual)
    }

    fn strict_diagnostics(actual: &str, expected: Option<&str>) -> Result<(), String> {
        compare_bytes("diagnostics", expected, actual)
    }

    #[test]
    fn missing_out_requires_empty_stdout() {
        assert!(
            strict_stdout("", None).is_ok(),
            "empty stdout with no .out is fine"
        );
        let err = strict_stdout("hello\n", None).expect_err("printing without .out must fail");
        assert!(err.contains("stdout mismatch"), "got: {err}");
        assert!(err.contains("hello"), "got: {err}");
    }

    #[test]
    fn missing_err_requires_zero_diagnostics() {
        assert!(
            strict_diagnostics("", None).is_ok(),
            "no diagnostics with no .err is fine"
        );
        let err = strict_diagnostics("E0101:2:5: unexpected token\n", None)
            .expect_err("diagnostics without .err must fail");
        assert!(err.contains("diagnostics mismatch"), "got: {err}");
        assert!(err.contains("E0101:2:5"), "got: {err}");
    }

    #[test]
    fn present_out_must_match_exactly() {
        assert!(strict_stdout("ok\n", Some("ok\n")).is_ok());
        // A trailing newline difference is a failure, not a rounding error.
        assert!(strict_stdout("ok\n", Some("ok")).is_err());
        assert!(strict_stdout("ok", Some("ok\n")).is_err());
    }

    #[test]
    fn exit_code_check_accepts_matching_status() {
        let output = command_output(&["version"]);
        let mut problems = Vec::new();
        check_exit_code(
            &header(&["version"], 0),
            &output,
            Path::new("t.out"),
            &mut problems,
        );
        assert!(problems.is_empty(), "got: {problems:?}");
    }

    #[test]
    fn exit_code_check_rejects_matching_status_when_header_disagrees() {
        let output = command_output(&["version"]);
        let mut problems = Vec::new();
        check_exit_code(
            &header(&["version"], 1),
            &output,
            Path::new("t.out"),
            &mut problems,
        );
        assert_eq!(problems.len(), 1, "a wrong expectation must be reported");
        assert!(problems[0].contains("exit status"), "got: {}", problems[0]);
        assert!(problems[0].contains("expected: 1"), "got: {}", problems[0]);
    }

    #[test]
    fn an_unknown_subcommand_actually_exits_nonzero() {
        // Guards the `// exit: N` feature against a CLI that always exits 0.
        let output = command_output(&["definitely-not-a-subcommand"]);
        assert_eq!(output.status.code(), Some(2));
    }

    /// Runs the `orv` binary under test with `argv[0]` as the subcommand.
    fn command_output(argv: &[&str]) -> Output {
        let argv: Vec<String> = argv.iter().map(|s| (*s).to_owned()).collect();
        run_command(&orv_binary(), &argv).expect("running the orv binary")
    }
}
