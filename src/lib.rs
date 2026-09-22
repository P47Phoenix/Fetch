//! fetch-mcp library (A-2 stdio server, A-3a SSRF core, A-3b guarded fetch): config, errors, stderr logger,
//! fail-closed `Policy`, the `ssrf` module (range table, URL and IP-literal checks, resolver filter), the
//! `fetch` module (streaming, size-bounded HTTP client that dials only SSRF-validated addresses) and the
//! stdio MCP `fetch` tool.
//! Stdout is reserved for MCP protocol frames (FR-13): application code never prints to it.
#![deny(clippy::print_stdout)]

pub mod config;
pub mod convert;
pub mod error;
pub mod fetch;
pub mod obs;
pub mod policy;
pub mod robots;
pub mod server;
pub mod ssrf;

/// Marker strings embedded only when a forbidden feature is compiled in.
/// Contract (architecture 9 R12, ADR-003, EPICS E-8): each marker starts with the fixed token
/// `FETCH_MCP_MARKER_<FEATURE>_V1` (what the release guard greps) followed by `:<feature-name>`, so
/// `--version` on a bench build contains the literal `bench-loopback` (what the harness checks).
#[must_use]
pub fn build_markers() -> Vec<&'static str> {
    // Each literal is cfg-gated, so a disabled feature's marker string is not compiled in at all.
    [
        #[cfg(feature = "test-support")]
        "FETCH_MCP_MARKER_TEST_SUPPORT_V1:test-support",
        #[cfg(feature = "bench-loopback")]
        "FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback",
    ]
    .to_vec()
}

/// Git commit the binary was built from (`unknown` outside a git checkout); set by `build.rs`.
pub const COMMIT: &str = env!("FETCH_MCP_COMMIT");
/// SHA-256 (hex) of `Cargo.lock` at build time; set by `build.rs`.
pub const CARGO_LOCK_SHA256: &str = env!("FETCH_MCP_CARGO_LOCK_SHA256");

/// The `--version` line: crate name and version, `commit=<sha>` and `cargo-lock=<sha256>` (E-4, re-homed from
/// E-8: the shipped and bench builds of one commit must report the same pair), then one marker per forbidden
/// feature compiled in (none in a release build).
#[must_use]
pub fn version_line() -> String {
    let mut line = format!(
        "fetch-mcp {} commit={COMMIT} cargo-lock={CARGO_LOCK_SHA256}",
        env!("CARGO_PKG_VERSION")
    );
    for m in build_markers() {
        line.push(' ');
        line.push_str(m);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::{build_markers, version_line};

    #[test]
    fn default_build_has_no_markers() {
        if !cfg!(any(feature = "test-support", feature = "bench-loopback")) {
            assert!(build_markers().is_empty());
        }
    }

    #[test]
    fn version_line_has_crate_version() {
        assert!(version_line().starts_with(concat!("fetch-mcp ", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn version_line_carries_commit_and_lock_hash() {
        let v = version_line();
        assert!(v.contains(&format!(" commit={}", super::COMMIT)));
        assert!(v.contains(&format!(" cargo-lock={}", super::CARGO_LOCK_SHA256)));
        assert_eq!(super::CARGO_LOCK_SHA256.len(), 64);
        assert!(super::CARGO_LOCK_SHA256
            .bytes()
            .all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn embedded_lock_hash_matches_cargo_lock() {
        // Independent check of build.rs's SHA-256: compare with the system tool when it exists.
        let out = std::process::Command::new("sha256sum")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"))
            .output()
            .ok();
        if let Some(o) = out.filter(|o| o.status.success()) {
            let s = String::from_utf8(o.stdout).unwrap();
            assert_eq!(
                s.split_whitespace().next().unwrap(),
                super::CARGO_LOCK_SHA256
            );
        }
    }

    #[cfg(feature = "bench-loopback")]
    #[test]
    fn bench_build_version_carries_marker() {
        let v = version_line();
        assert!(v.contains("FETCH_MCP_MARKER_BENCH_LOOPBACK_V1"));
        assert!(v.contains("bench-loopback"));
    }
}
