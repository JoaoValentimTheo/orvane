//! Orvane command line interface (`orv`).
//!
//! M0 implemented `orv version`; M1 adds the hidden `orv tokens` debug
//! subcommand. The remaining subcommands from SPEC §10 arrive with their
//! milestones; the budget is 8 subcommands total.

#![forbid(unsafe_code)]

mod cli;
mod diag;
mod tokens;

fn main() -> std::process::ExitCode {
    cli::run()
}
