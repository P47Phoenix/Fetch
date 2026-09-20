//! Parser differential test (architect N2a, EPICS A-3b merge gate). `check_url` hand-rolls host parsing, while
//! the HTTP client re-parses the URL with the `url` crate (WHATWG) for the Host header and SNI. This test runs
//! both over an encodings corpus and a deterministic fuzz corpus and asserts the one property that matters:
//!
//! * (A) nothing the reference parser sees as a blocked host is accepted by `check_url`;
//! * (B) whatever `check_url` accepts, the reference parser reads as the same scheme, port and host;
//! * (C) userinfo the reference sees is never accepted.
//!
//! `check_url` refusing something the reference accepts is fine (fail closed). The reference is a
//! dev-dependency only; production code does not link it directly (it is reqwest's transitive `url`).

use super::{check_url, CheckedUrl, Host, Origin};
use crate::error::FetchError;
use crate::policy::Policy;
use std::net::IpAddr;
use url::Url;

/// Returns a description of a violated property, or `None`.
fn disagreement(input: &str) -> Option<String> {
    judge(input, check_url(input, &Policy::default(), Origin::Initial))
}

/// The three properties, applied to `ours` (injectable so the positive control can feed a broken outcome).
fn judge(input: &str, ours: Result<CheckedUrl, FetchError>) -> Option<String> {
    let policy = Policy::default();
    let reference = Url::parse(input)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"));

    // (A) a host the reference sees as blocked must be refused by us.
    if let Some(u) = &reference {
        let blocked_host = match u.host() {
            Some(url::Host::Ipv4(a)) => policy.check_ip(IpAddr::V4(a)).is_err(),
            Some(url::Host::Ipv6(a)) => policy.check_ip(IpAddr::V6(a)).is_err(),
            Some(url::Host::Domain(d)) => {
                let d = d.trim_end_matches('.').to_ascii_lowercase();
                d == "localhost" || d.ends_with(".localhost")
            }
            None => false,
        };
        if blocked_host && ours.is_ok() {
            return Some(format!(
                "BLOCKED HOST PASSED: {input:?} -> {ours:?} (reference host {:?})",
                u.host()
            ));
        }
        // (C) userinfo.
        if (!u.username().is_empty() || u.password().is_some()) && ours.is_ok() {
            return Some(format!("USERINFO PASSED: {input:?} -> {ours:?}"));
        }
    }

    // (B) agreement on everything accepted.
    if let Ok(c) = &ours {
        let Some(u) = &reference else {
            return Some(format!(
                "ACCEPTED but reference rejects: {input:?} -> {c:?}"
            ));
        };
        if (u.scheme() == "https") != c.https {
            return Some(format!(
                "scheme differs: {input:?} -> {c:?} vs {}",
                u.scheme()
            ));
        }
        if u.port_or_known_default() != Some(c.port) {
            return Some(format!(
                "port differs: {input:?} -> {c:?} vs {:?}",
                u.port_or_known_default()
            ));
        }
        let same_host = match (&c.host, u.host()) {
            (Host::Ip(IpAddr::V4(a)), Some(url::Host::Ipv4(b))) => *a == b,
            (Host::Ip(IpAddr::V6(a)), Some(url::Host::Ipv6(b))) => *a == b,
            (Host::Name(n), Some(url::Host::Domain(d))) => {
                d.trim_end_matches('.').eq_ignore_ascii_case(n)
            }
            _ => false,
        };
        if !same_host {
            return Some(format!(
                "host differs: {input:?} -> {:?} vs {:?}",
                c.host,
                u.host()
            ));
        }
    }
    None
}

/// Non-vacuity counters, so a corpus that never exercises the properties cannot pass unnoticed.
#[derive(Default)]
struct Stats {
    total: usize,
    accepted_by_us: usize,
    blocked_by_reference: usize,
}

fn assert_no_disagreement(inputs: impl IntoIterator<Item = String>) -> Stats {
    let mut st = Stats::default();
    let mut bad = Vec::new();
    for i in inputs {
        st.total += 1;
        if check_url(&i, &Policy::default(), Origin::Initial).is_ok() {
            st.accepted_by_us += 1;
        }
        if let Some(u) = Url::parse(&i)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
        {
            let blocked = match u.host() {
                Some(url::Host::Ipv4(a)) => Policy::default().check_ip(IpAddr::V4(a)).is_err(),
                Some(url::Host::Ipv6(a)) => Policy::default().check_ip(IpAddr::V6(a)).is_err(),
                _ => false,
            };
            st.blocked_by_reference += usize::from(blocked);
        }
        if let Some(d) = disagreement(&i) {
            bad.push(d);
        }
    }
    assert!(
        bad.is_empty(),
        "{} disagreements, first 20:\n{}",
        bad.len(),
        bad.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
    st
}

#[test]
fn encodings_corpus() {
    let hosts = [
        // loopback / private / metadata in every spelling
        "127.0.0.1",
        "127.1",
        "127.0.1",
        "0x7f.1",
        "0x7f000001",
        "2130706433",
        "017700000001",
        "0177.0.0.1",
        "0x7F.0X0.0.1",
        "127.0.0.1.",
        "127。0。0。1",
        "１２７.0.0.1",
        "%31%32%37.0.0.1",
        "127.0.0.0x1",
        "0.0.0.0",
        "0",
        "0x0",
        "00",
        "[::]",
        "[::1]",
        "[0:0:0:0:0:0:0:1]",
        "[::ffff:127.0.0.1]",
        "[::ffff:7f00:1]",
        "[::127.0.0.1]",
        "[64:ff9b::7f00:1]",
        "[2002:7f00:1::]",
        "[::ffff:0:7f00:1]",
        "[0:0:0:0:0:ffff:127.0.0.1]",
        "10.0.0.1",
        "10.1",
        "0xa000001",
        "012.0.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "3232235777",
        "169.254.169.254",
        "0xa9fea9fe",
        "2852039166",
        "025177524776",
        "169.254.43518",
        "169.16689662",
        "[fd00:ec2::254]",
        "[fe80::1]",
        "[fc00::1]",
        "100.64.0.1",
        "168.63.129.16",
        "localhost",
        "LOCALHOST",
        "localhost.",
        "a.localhost",
        "foo.LOCALHOST.",
        "localhost:80",
        // public and near-public
        "8.8.8.8",
        "8.8.8",
        "0x8.8.8.8",
        "[2606:4700:4700::1111]",
        "example.com",
        "EXAMPLE.com",
        "example.com.",
        "a.b.c.d.e",
        "xn--nxasmq6b.com",
        "1.1.1.1.1",
        "1.1.1.256",
        "256.256.256.256",
        "1.2.3",
        "1.2.3.4.5",
        "0x",
        "0x.0.0.1",
        "1..1",
        ".",
        "..",
        "a..b",
        "-a.com",
        "a_b.com",
        "999999999999",
        "4294967296",
        "0x100000000",
        // odd characters
        "a b",
        "a\tb",
        "a\nb",
        "a%20b",
        "a%2e",
        "a%00",
        "a\\b",
        "a;b",
        "a,b",
        "a*b",
        "a'b",
        "a!b",
        "[",
        "]",
        "[]",
        "[::1",
        "::1]",
        "[::1]x",
        "[::1]:",
        "[::1]:80",
        "[::1]:0",
        "[::1]:65536",
        "[::1%25eth0]",
        "[fe80::1%eth0]",
        "[1::2::3]",
        "[12345::]",
        "[::1.2.3]",
        "[::1.2.3.256]",
        "[1:2:3:4:5:6:7:8:9]",
    ];
    let suffixes = [
        "", "/", "/path", "?q=1", "#f", ":80", ":443", ":8080", ":0", ":65535", ":65536", ":080",
        ":+80", ":-1", ":8 0", ":",
    ];
    let prefixes = [
        "http://",
        "https://",
        "HTTP://",
        "HtTpS://",
        "http:/",
        "http:",
        "http:\\\\",
        "http:\\/",
        "http:/\\",
        "http:///",
        "http:////",
        " http://",
        "http://\t",
        "\thttp://",
        "http://\n",
        "http://user@",
        "http://user:pw@",
        "http://:@",
        "http://@",
        "http://a:b@c@",
        "http://\\",
        "https:\\\\",
        "ftp://",
        "file://",
        "javascript://",
        "gopher://",
        "//",
        "",
        "http://example.com\\@",
        "http://example.com:80@",
        "http://example.com#@",
        "http://example.com?@",
        "http://example.com/@",
    ];
    let mut all = Vec::new();
    for p in prefixes {
        for h in hosts {
            for s in suffixes {
                all.push(format!("{p}{h}{s}"));
            }
        }
    }
    let st = assert_no_disagreement(all);
    assert!(st.total > 10_000, "corpus too small: {}", st.total);
    assert!(
        st.accepted_by_us > 500,
        "property (B) barely exercised: {}",
        st.accepted_by_us
    );
    assert!(
        st.blocked_by_reference > 1_000,
        "property (A) barely exercised: {}",
        st.blocked_by_reference
    );
}

/// Every spelling of a set of IPv4 addresses (decimal, octal, hex, one to four parts, padded, mixed) must be
/// judged like the reference reads it: blocked addresses stay blocked, accepted ones are the same address.
#[test]
fn every_ipv4_spelling_is_judged_like_the_reference() {
    fn fmt(v: u64, radix: u8, pad: usize) -> String {
        match radix {
            8 => format!("0{:0>pad$o}", v, pad = pad),
            16 => format!("0x{:0>pad$x}", v, pad = pad),
            _ => format!("{v:0>pad$}", pad = pad),
        }
    }
    let addrs: [[u8; 4]; 14] = [
        [127, 0, 0, 1],
        [127, 255, 255, 254],
        [0, 0, 0, 0],
        [10, 0, 0, 1],
        [172, 16, 5, 4],
        [192, 168, 0, 1],
        [169, 254, 169, 254],
        [100, 64, 0, 1],
        [224, 0, 0, 1],
        [255, 255, 255, 255],
        [168, 63, 129, 16],
        [8, 8, 8, 8],
        [93, 184, 216, 34],
        [1, 2, 3, 4],
    ];
    let mut all = Vec::new();
    for a in addrs {
        let [b0, b1, b2, b3] = a.map(u64::from);
        let whole = (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
        for radix in [10u8, 8, 16] {
            for pad in [0usize, 1, 3, 12] {
                let f = |v: u64| fmt(v, radix, pad);
                all.push(f(whole));
                all.push(format!("{}.{}", f(b0), f((b1 << 16) | (b2 << 8) | b3)));
                all.push(format!("{}.{}.{}", f(b0), f(b1), f((b2 << 8) | b3)));
                all.push(format!("{}.{}.{}.{}", f(b0), f(b1), f(b2), f(b3)));
                all.push(format!("{}.{}.{}.{}.", f(b0), f(b1), f(b2), f(b3)));
            }
        }
        // mixed radix per part
        for (r0, r1, r2, r3) in [(10u8, 16u8, 8u8, 10u8), (16, 10, 10, 8), (8, 8, 16, 16)] {
            all.push(format!(
                "{}.{}.{}.{}",
                fmt(b0, r0, 0),
                fmt(b1, r1, 0),
                fmt(b2, r2, 0),
                fmt(b3, r3, 0)
            ));
        }
    }
    let urls: Vec<String> = all
        .iter()
        .flat_map(|h| [format!("http://{h}/"), format!("https://{h}:8443/x")])
        .collect();
    let st = assert_no_disagreement(urls);
    assert!(st.total > 1_500, "{}", st.total);
    assert!(
        st.blocked_by_reference > 500,
        "property (A) barely exercised: {}",
        st.blocked_by_reference
    );
    assert!(
        st.accepted_by_us > 50,
        "property (B) barely exercised: {}",
        st.accepted_by_us
    );
}

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Deterministic token-soup fuzz: structure-aware (schemes, delimiters, numbers, brackets, escapes).
#[test]
fn deterministic_fuzz_corpus() {
    let tokens = [
        "http://",
        "https://",
        "http:",
        "//",
        "/",
        "\\",
        "@",
        ":",
        ".",
        "..",
        "[",
        "]",
        "::",
        "?",
        "#",
        "%",
        "%2e",
        "%7f",
        "0",
        "1",
        "7",
        "8",
        "9",
        "10",
        "127",
        "169",
        "254",
        "255",
        "256",
        "0x",
        "0X7f",
        "x",
        "a",
        "z",
        "-",
        "_",
        " ",
        "\t",
        "\n",
        "localhost",
        "example",
        "com",
        "ffff",
        "fe80",
        "fd00",
        "::ffff:",
        "1.2.3.4",
        "0177",
        "2130706433",
        "80",
        "443",
        "65535",
        "65536",
        "user",
        "pw",
        "。",
        "１",
        "é",
        "%00",
        ";",
        "=",
        "&",
        "+",
        "!",
        "*",
        "'",
        "(",
        ")",
    ];
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut inputs = Vec::new();
    for _ in 0..150_000 {
        let mut s = String::from(if xorshift(&mut state).is_multiple_of(8) {
            "https://"
        } else {
            "http://"
        });
        for _ in 0..(1 + xorshift(&mut state) % 10) {
            s.push_str(tokens[(xorshift(&mut state) as usize) % tokens.len()]);
        }
        inputs.push(s);
    }
    let st = assert_no_disagreement(inputs);
    assert!(
        st.accepted_by_us > 1_000 && st.blocked_by_reference > 1_000,
        "fuzz corpus too narrow"
    );
}

/// Positive control: the properties must be able to fail. Broken outcomes for a parser that lets a blocked host
/// through, gets the port wrong, or drops userinfo are each flagged.
#[test]
fn checker_flags_a_broken_parser() {
    let ip = |s: &str| Host::Ip(s.parse().unwrap());
    let ok = |https, host, port| Ok(CheckedUrl { https, host, port });
    // (A) blocked host accepted, in a spelling only a WHATWG parser folds.
    assert!(judge("http://2130706433/", ok(false, ip("127.0.0.1"), 80)).is_some());
    assert!(judge(
        "http://localhost/",
        ok(false, Host::Name("localhost".into()), 80)
    )
    .is_some());
    // (B) wrong port, wrong scheme, wrong host, accepted although the reference rejects.
    assert!(judge("http://8.8.8.8:81/", ok(false, ip("8.8.8.8"), 80)).is_some());
    assert!(judge("https://8.8.8.8/", ok(false, ip("8.8.8.8"), 443)).is_some());
    assert!(judge("http://8.8.8.8/", ok(false, ip("8.8.4.4"), 80)).is_some());
    assert!(judge("http://a b/", ok(false, Host::Name("a b".into()), 80)).is_some());
    // (C) userinfo accepted.
    assert!(judge(
        "http://u:p@example.com/",
        ok(false, Host::Name("example.com".into()), 80)
    )
    .is_some());
    // and the honest outcomes are not flagged.
    assert!(judge("http://8.8.8.8/", ok(false, ip("8.8.8.8"), 80)).is_none());
    assert!(judge(
        "http://2130706433/",
        Err(FetchError::BlockedTarget("x".into()))
    )
    .is_none());
}
