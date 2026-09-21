//! Orvane command line interface (`orv`).
//!
//! M0 implements `orv version` only. The remaining subcommands from SPEC §10
//! arrive with their milestones; the budget is 8 subcommands total.

#![forbid(unsafe_code)]

mod cli;

fn main() -> std::process::ExitCode {
    cli::run()
}
