//! Orvane semantic analysis crate.
//!
//! Planned for M3 (name resolution and type checking), M5 (`data`/`enum`/
//! `match`) and M6 (intent checks, `IntentTable`). M0 ships the crate skeleton
//! only, so the workspace layout from SPEC §3.2 exists and compiles.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    /// The crate is intentionally empty in M0: this test pins the workspace
    /// wiring so `cargo test --workspace` exercises every member.
    #[test]
    fn crate_is_wired() {
        let name = env!("CARGO_PKG_NAME");
        assert_eq!(name, "orv-sema");
    }
}
