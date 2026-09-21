//! End-to-end golden harness (SPEC §11 item 2, ADR 0003).
//!
//! It walks `tests/golden/` and, for every `<name>.orv`, runs the command
//! declared in the file's header and compares the result with the checked-in
//! expectation files:
//!
//! ```text
//! // orv: <subcommand> [args...]     <- header, first non-empty line
//! ```
//!
//! * `<name>.out` — expected **stdout**, compared byte for byte.
//! * `<name>.err` — expected **diagnostics**, compared against the normalized
//!   `CODE:linha:coluna: mensagem` lines from stderr (SPEC §8.1). The pretty
//!   `ariadne` drawing is deliberately ignored so the expectation stays stable.
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

    let argv = parse_header(&source).ok_or_else(|| {
        format!("{name}: missing `// orv: <subcommand>` header on the first non-empty line")
    })?;

    let output = run_command(orv_bin, &argv)
        .map_err(|err| format!("{name}: cannot run `{orv_bin:?} {}`: {err}", argv.join(" ")))?;

    let stem = orv_file.with_extension("");
    let mut problems: Vec<String> = Vec::new();

    let out_path = stem.with_extension("out");
    if out_path.exists() {
        let expected = read_expectation(&out_path)?;
        let actual = decode_output(&output.stdout);
        if actual != expected {
            problems.push(diff_report("stdout", &expected, &actual));
        }
    } else {
        problems.push(format!(
            "{name}: missing {} (every golden case needs an expectation)",
            file_label(&out_path)
        ));
    }

    let err_path = stem.with_extension("err");
    if err_path.exists() {
        let expected = read_expectation(&err_path)?;
        let actual = normalize_diagnostics(&decode_output(&output.stderr));
        if actual != expected {
            problems.push(diff_report("diagnostics", &expected, &actual));
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Reads the first non-empty line and extracts the command after `// orv:`.
fn parse_header(source: &str) -> Option<Vec<String>> {
    let line = source.lines().find(|l| !l.trim().is_empty())?;
    let rest = line.trim().strip_prefix("//")?.trim();
    let rest = rest.strip_prefix("orv:")?;
    let argv: Vec<String> = rest.split_whitespace().map(str::to_owned).collect();
    if argv.is_empty() { None } else { Some(argv) }
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

fn run_command(orv_bin: &Path, argv: &[String]) -> io::Result<Output> {
    let Some((subcommand, args)) = argv.split_first() else {
        return Err(io::Error::other("empty command header"));
    };
    Command::new(orv_bin).arg(subcommand).args(args).output()
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

    #[test]
    fn header_parsing_ignores_blank_lines() {
        let src = "\n\n// orv: version\norvane\n";
        assert_eq!(parse_header(src), Some(vec!["version".to_owned()]));
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
            Some(vec!["run".to_owned(), "examples/fib.orv".to_owned()])
        );
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
}
