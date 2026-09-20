//! fetch-mcp binary. `--version` prints the crate version plus a marker for every forbidden feature compiled
//! in (keeps the marker strings in the artifact so the release guard's grep is reliable). Otherwise serves MCP
//! over stdio on a `current_thread` runtime. Stdout carries only protocol frames; logs go to stderr.

use fetch_mcp::{config::Config, obs, policy::Policy, server::Fetch};
use rmcp::ServiceExt;

#[allow(clippy::print_stdout)] // --version is the one deliberate stdout print, and it exits before any MCP traffic
fn print_version() {
    println!("{}", fetch_mcp::version_line());
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    if std::env::args().any(|a| a == "--version") {
        print_version();
        return std::process::ExitCode::SUCCESS;
    }
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            obs::stderr_line(format_args!("error config {e}"));
            return std::process::ExitCode::from(2);
        }
    };
    obs::log(
        cfg.log_level,
        obs::Level::Info,
        "startup",
        format_args!("version={}", env!("CARGO_PKG_VERSION")),
    );
    match serve().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            // rmcp's error text can echo the client's first frame verbatim; keep that out of stderr unless the
            // operator opted into debug logging.
            if obs::enabled(cfg.log_level, obs::Level::Debug) {
                obs::stderr_line(format_args!("error serve {e}"));
            } else {
                obs::stderr_line(format_args!(
                    "error serve session ended abnormally (details withheld; FETCH_LOG=debug shows them)"
                ));
            }
            std::process::ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let svc = Fetch::new(Policy::default())
        .serve(rmcp::transport::stdio())
        .await?;
    svc.waiting().await?;
    Ok(())
}
