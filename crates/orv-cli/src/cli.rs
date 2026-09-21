//! Argument parsing and command dispatch for `orv`.
//!
//! Exit codes follow SPEC §10: `0` ok, `1` program/diagnostic error, `2`
//! environment or usage error, `130` interrupted.
//!
//! Output uses explicit `LF` line endings instead of `println!`, so the bytes
//! on stdout do not depend on the platform (`println!` emits `CRLF` on
//! Windows). Golden expectations are LF-only, and the harness must not be the
//! only thing papering over the difference — see ADR 0002.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// The `orv` command line.
#[derive(Debug, Parser)]
#[command(
    name = "orv",
    version,
    about = "Orvane: say what needs to happen. Orvane chooses how.",
    long_about = None,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Every subcommand the CLI exposes.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the Orvane version.
    Version,
    /// Debug: dump the token stream of a file.
    #[command(hide = true)]
    Tokens {
        /// The `.orv` file to lex.
        file: PathBuf,
    },
}

/// How a subcommand finished, mapped to the SPEC §10 exit codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubcommandOutcome {
    /// `0`: success.
    Ok,
    /// `1`: program error or diagnostics were reported.
    Failure,
    /// `2`: environment or usage error.
    Usage,
}

impl SubcommandOutcome {
    /// The documented exit code.
    pub const fn exit_code(self) -> u8 {
        match self {
            SubcommandOutcome::Ok => 0,
            SubcommandOutcome::Failure => 1,
            SubcommandOutcome::Usage => 2,
        }
    }
}

/// Parses arguments, runs the requested command and returns an exit code.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let outcome = match cli.command {
        Command::Version => match print_lf(&version_line()) {
            Ok(()) => SubcommandOutcome::Ok,
            Err(_) => SubcommandOutcome::Usage,
        },
        Command::Tokens { file } => crate::tokens::run(&file),
    };
    ExitCode::from(outcome.exit_code())
}

/// Writes `line` to stdout followed by a single `LF`.
///
/// Deliberately not `println!`: that macro's newline is platform-dependent
/// (`CRLF` on Windows), and golden tests compare raw bytes.
fn print_lf(line: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(line.as_bytes())?;
    lock.write_all(b"\n")?;
    lock.flush()
}

/// The single line printed by `orv version`.
///
/// Format: `orv <crate version> (orvane <crate version>)`.
pub fn version_line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!("orv {version} (orvane {version})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_line_matches_crate_version() {
        assert_eq!(
            version_line(),
            format!(
                "orv {} (orvane {})",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn version_subcommand_parses() {
        let cli = Cli::try_parse_from(["orv", "version"]).expect("parses");
        assert!(matches!(cli.command, Command::Version));
    }

    #[test]
    fn tokens_subcommand_parses_with_a_file() {
        let cli = Cli::try_parse_from(["orv", "tokens", "x.orv"]).expect("parses");
        match cli.command {
            Command::Tokens { file } => assert_eq!(file, PathBuf::from("x.orv")),
            other => panic!("expected Tokens, got {other:?}"),
        }
    }

    #[test]
    fn tokens_requires_a_file_argument() {
        assert!(Cli::try_parse_from(["orv", "tokens"]).is_err());
    }

    #[test]
    fn unknown_subcommand_is_a_usage_error() {
        let err = Cli::try_parse_from(["orv", "nope"]).expect_err("rejects");
        // SPEC §10: usage errors exit with code 2.
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn no_subcommand_is_a_usage_error() {
        assert!(Cli::try_parse_from(["orv"]).is_err());
    }

    #[test]
    fn outcome_exit_codes_match_spec() {
        assert_eq!(SubcommandOutcome::Ok.exit_code(), 0);
        assert_eq!(SubcommandOutcome::Failure.exit_code(), 1);
        assert_eq!(SubcommandOutcome::Usage.exit_code(), 2);
    }
}
