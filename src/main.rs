//! fetch-mcp binary skeleton (D-7). `--version` prints the crate version plus a marker for every
//! forbidden feature compiled in, which keeps the marker strings in the artifact so the release
//! guard's grep is reliable (a clean release build prints no marker).

fn main() {
    if std::env::args().any(|a| a == "--version") {
        println!("{}", fetch_mcp::version_line());
    }
}
