//! fetch-mcp library skeleton (D-7). Fetch, SSRF and MCP logic arrive in A-2/A-3a.

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

/// The `--version` line: crate name and version, then one marker per forbidden feature compiled in
/// (none in a release build). Commit and Cargo.lock hash are added by E-8/D-3 (build script).
#[must_use]
pub fn version_line() -> String {
    let mut line = format!("fetch-mcp {}", env!("CARGO_PKG_VERSION"));
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

    #[cfg(feature = "bench-loopback")]
    #[test]
    fn bench_build_version_carries_marker() {
        let v = version_line();
        assert!(v.contains("FETCH_MCP_MARKER_BENCH_LOOPBACK_V1"));
        assert!(v.contains("bench-loopback"));
    }
}
