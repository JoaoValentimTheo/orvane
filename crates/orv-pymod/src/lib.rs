//! Orvane's Python extension module (`orvane`), built with maturin.
//!
//! `orvane.load` / `orvane.run` land in M8. M0 ships a compilable skeleton with
//! no `pyo3` dependency yet, so the workspace builds without a Python
//! toolchain (SPEC §3.2 keeps `pyo3` out of the core crates anyway).

#![forbid(unsafe_code)]

/// The crate's Python module name, as declared in SPEC §1.1.
pub const MODULE_NAME: &str = "orvane";

#[cfg(test)]
mod tests {
    use super::MODULE_NAME;

    #[test]
    fn crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "orv-pymod");
        assert_eq!(MODULE_NAME, "orvane");
    }
}
