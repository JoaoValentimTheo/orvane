//! Orvane Python host crate (PyO3).
//!
//! This is one of the only two crates allowed to depend on `pyo3` (SPEC
//! §3.2/§3.3). The `PyHost` implementation lands in M7; M0 ships the skeleton
//! with no `pyo3` dependency yet, keeping `cargo test --workspace` buildable
//! without a Python toolchain.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    /// The crate is intentionally empty in M0: this test pins the workspace
    /// wiring so `cargo test --workspace` exercises every member.
    #[test]
    fn crate_is_wired() {
        let name = env!("CARGO_PKG_NAME");
        assert_eq!(name, "orv-py");
    }
}
