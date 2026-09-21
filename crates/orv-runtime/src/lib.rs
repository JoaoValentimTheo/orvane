//! Orvane runtime crate.
//!
//! Planned for M4 (`Value`, tree-walking interpreter), M6 (`Planner`, `Trace`)
//! and M7 (`Host`, `NoHost`, `MockHost`). M0 ships the crate skeleton only.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    /// The crate is intentionally empty in M0: this test pins the workspace
    /// wiring so `cargo test --workspace` exercises every member.
    #[test]
    fn crate_is_wired() {
        let name = env!("CARGO_PKG_NAME");
        assert_eq!(name, "orv-runtime");
    }
}
