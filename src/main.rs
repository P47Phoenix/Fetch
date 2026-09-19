//! fetch-mcp binary skeleton (D-7). The marker print keeps feature markers in the
//! artifact so the release guard's string grep is reliable.

fn main() {
    if std::env::args().any(|a| a == "--build-info") {
        let markers = fetch_mcp::build_markers();
        std::process::exit(if markers.is_empty() {
            0
        } else {
            i32::try_from(markers.len()).unwrap_or(1)
        });
    }
}
