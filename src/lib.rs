//! fetch-mcp library skeleton (D-7). Fetch, SSRF and MCP logic arrive in A-2/A-3a.

/// Marker strings embedded only when a forbidden feature is compiled in.
/// The release guard greps the built artifact for these (architecture R12).
#[must_use]
pub fn build_markers() -> Vec<&'static str> {
    #[allow(unused_mut)]
    let mut markers = Vec::new();
    #[cfg(feature = "test-support")]
    markers.push("FETCH_MCP_MARKER_TEST_SUPPORT_V1");
    #[cfg(feature = "bench-loopback")]
    markers.push("FETCH_MCP_MARKER_BENCH_LOOPBACK_V1");
    #[cfg(feature = "fixture-ca")]
    markers.push("FETCH_MCP_MARKER_FIXTURE_CA_V1");
    markers
}

#[cfg(test)]
mod tests {
    use super::build_markers;

    #[test]
    fn default_build_has_no_markers() {
        if !cfg!(any(
            feature = "test-support",
            feature = "bench-loopback",
            feature = "fixture-ca"
        )) {
            assert!(build_markers().is_empty());
        }
    }
}
