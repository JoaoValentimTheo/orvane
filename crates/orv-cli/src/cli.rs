//! Argument parsing and command dispatch for `orv`.
//!
//! Exit codes follow SPEC §10: `0` ok, `1` program/diagnostic error, `2`
//! environment or usage error, `130` interrupted.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Exit code for a successful run (SPEC §10).
///
/// The remaining documented codes (`1` program error, `2` usage/environment,
/// `130` interrupted) are introduced by the milestones that can produce them,
/// to keep M0 free of unused code.
pub const EXIT_OK: u8 = 0;

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

/// Every subcommand the CLI exposes. M0 supports `version` only.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the Orvane version.
    Version,
}

/// Parses arguments, runs the requested command and returns an exit code.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("{}", version_line());
            ExitCode::from(EXIT_OK)
        }
    }
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
    fn unknown_subcommand_is_a_usage_error() {
        let err = Cli::try_parse_from(["orv", "nope"]).expect_err("rejects");
        // SPEC §10: usage errors exit with code 2.
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn ok_exit_code_matches_spec() {
        assert_eq!(EXIT_OK, 0);
    }

    #[test]
    fn no_subcommand_is_a_usage_error() {
        assert!(Cli::try_parse_from(["orv"]).is_err());
    }
}
