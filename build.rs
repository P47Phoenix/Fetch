//! Build identity (E-4, re-homed from E-8): embeds the git commit and the SHA-256 of Cargo.lock so `--version`
//! reports what was built. Deterministic: no timestamps, no host paths, no new dependency (SHA-256 is written out
//! below). The shipped and bench-loopback binaries are built from one tree, so both must report the same pair.
//! Outside a git checkout (for example `git archive`) the commit is `unknown`; `FETCH_MCP_COMMIT` overrides it (trusted,
//! unchecked). A tree with modified tracked files reports `<sha>-dirty`, which the gate tooling refuses.
use std::{fs, path::Path, process::Command};

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, b) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// Run `git` with `args`; the trimmed stdout on success.
fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// The commit `--version` reports: `FETCH_MCP_COMMIT` verbatim when set (an escape hatch for builds with no `.git`, for
/// example `git archive` or a Docker context: the value is TRUSTED, nothing here can check it, so the identity gate proves
/// "both binaries claim the same string", not that the string describes the source); otherwise `git rev-parse HEAD`
/// (40 hex), with `-dirty` appended when a TRACKED file differs from HEAD (untracked files, such as CI's `out/`, do not
/// count). The gate tooling matches 40 hex only, so it refuses a `-dirty` build. `unknown` outside a git checkout.
fn git_commit() -> String {
    if let Ok(c) = std::env::var("FETCH_MCP_COMMIT") {
        if !c.is_empty() {
            return c;
        }
    }
    let Some(head) = git(&["rev-parse", "--verify", "HEAD"])
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return "unknown".to_string();
    };
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).is_none_or(|s| !s.is_empty());
    if dirty {
        format!("{head}-dirty")
    } else {
        head
    }
}

/// Re-run the script when the checked-out commit or the tracked sources change. Works for worktrees and submodules
/// (`.git` is then a file; `git rev-parse` resolves the real git dir) and for packed refs.
fn rerun_on_git_changes() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let (Some(dir), Some(common)) = (
        git(&["rev-parse", "--git-dir"]),
        git(&["rev-parse", "--git-common-dir"]),
    ) else {
        return; // no git: nothing to watch, the commit is `unknown` or FETCH_MCP_COMMIT
    };
    for p in [
        format!("{dir}/HEAD"),
        format!("{dir}/index"),
        format!("{common}/packed-refs"),
    ] {
        if Path::new(&p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"]) {
        let p = format!("{common}/{r}");
        if Path::new(&p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FETCH_MCP_COMMIT");
    rerun_on_git_changes();
    let lock = fs::read("Cargo.lock")
        .map(|d| sha256_hex(&d))
        .unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=FETCH_MCP_COMMIT={}", git_commit());
    println!("cargo:rustc-env=FETCH_MCP_CARGO_LOCK_SHA256={lock}");
}
