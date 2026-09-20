//! SSRF core (A-3a; architecture 14.1, ADR-003): `check_url` (scheme, userinfo, host and IP-literal checks,
//! before any resolution), the resolver filter ([`resolver`]) and per-hop redirect revalidation. Pure logic over
//! `std::net`: no HTTP client, no socket, no DNS here. The real resolver is injected by A-3b.
//!
//! Host parsing is deliberately strict and hand-rolled (no `url` crate, NFR-05): it accepts only what it can
//! judge, and anything ambiguous is refused (fail closed). Every IPv4 spelling the WHATWG URL parser folds to an
//! address (decimal, octal, hex, 1 to 4 parts) is folded the same way here and then checked as an address, so a
//! client that later re-parses the URL with a WHATWG parser dials the address that was validated.

pub mod ranges;
pub mod resolver;

use crate::error::FetchError;
use crate::policy::Policy;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Which URL is being checked. The initial `url` argument is a parameter (invalid-params on a bad scheme or
/// userinfo, ADR-006); a redirect `Location` is a network event (`blocked_target`, ADR-003 step 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Initial,
    Redirect,
}

/// The host of a checked URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    /// An IP literal in any spelling, already checked against the policy.
    Ip(IpAddr),
    /// A lower-case DNS name without trailing dot, still to go through the resolver filter.
    Name(String),
}

/// A URL that passed [`check_url`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUrl {
    pub https: bool,
    pub host: Host,
    pub port: u16,
}

fn reject(origin: Origin, field_msg: &str) -> FetchError {
    match origin {
        Origin::Initial => FetchError::InvalidArgument {
            field: "url",
            message: field_msg.to_string(),
        },
        Origin::Redirect => {
            FetchError::BlockedTarget(format!("redirect target refused: {field_msg}"))
        }
    }
}

/// Check a URL before any resolution or connection: scheme is `http` or `https`; no userinfo; a well-formed
/// host; a non-zero port; IP-literal hosts (any spelling, bracketed IPv6) judged by the policy now.
///
/// # Errors
/// `InvalidArgument` (initial URL) or `BlockedTarget` (redirect) for a bad scheme, userinfo, zone id or
/// malformed host; `BlockedTarget` for a blocked IP literal or a `localhost` name under a fail-closed policy.
pub fn check_url(url: &str, policy: &Policy, origin: Origin) -> Result<CheckedUrl, FetchError> {
    let bad = |m: &str| reject(origin, m);
    let Some((scheme, rest)) = url.split_once(':') else {
        return Err(bad("must be an absolute http or https URL"));
    };
    let https = if scheme.eq_ignore_ascii_case("https") {
        true
    } else if scheme.eq_ignore_ascii_case("http") {
        false
    } else {
        return Err(bad("scheme must be http or https"));
    };
    let Some(after) = rest.strip_prefix("//") else {
        return Err(bad("must be an absolute http or https URL with a host"));
    };
    // The authority ends at the first of / ? # or a backslash (which WHATWG treats as a slash).
    let end = after.find(['/', '?', '#', '\\']).unwrap_or(after.len());
    let authority = &after[..end];
    if authority.is_empty() {
        return Err(bad("host is empty"));
    }
    if authority.contains('@') {
        return Err(bad("userinfo (user:password@) is not allowed"));
    }
    if authority.bytes().any(|b| b <= 0x20 || b >= 0x7f) {
        return Err(bad(
            "host contains whitespace, control or non-ASCII characters",
        ));
    }
    let (host_str, port) = split_host_port(authority, https).map_err(bad)?;
    let host = parse_host(host_str).map_err(bad)?;
    match &host {
        Host::Ip(ip) => {
            if let Err(b) = policy.check_ip(*ip) {
                return Err(FetchError::BlockedTarget(format!(
                    "IP address is not public ({})",
                    b.category
                )));
            }
        }
        Host::Name(n) => {
            if !policy.allows_loopback() && (n == "localhost" || n.ends_with(".localhost")) {
                return Err(FetchError::BlockedTarget(
                    "hostname is not public (loopback)".to_string(),
                ));
            }
        }
    }
    Ok(CheckedUrl { https, host, port })
}

fn split_host_port(authority: &str, https: bool) -> Result<(&str, u16), &'static str> {
    let default = if https { 443 } else { 80 };
    let (host, port_str) = if authority.starts_with('[') {
        let close = authority.find(']').ok_or("unterminated IPv6 bracket")?;
        let after = &authority[close + 1..];
        let port = match after {
            "" => "",
            _ => after.strip_prefix(':').ok_or("junk after IPv6 address")?,
        };
        (&authority[..=close], port)
    } else {
        match authority.split_once(':') {
            Some((h, p)) => (h, p),
            None => (authority, ""),
        }
    };
    let port = if port_str.is_empty() {
        default
    } else if port_str.len() <= 5 && port_str.bytes().all(|b| b.is_ascii_digit()) {
        match port_str.parse::<u16>() {
            Ok(0) => return Err("port 0 is not allowed"),
            Ok(p) => p,
            Err(_) => return Err("port out of range"),
        }
    } else {
        return Err("invalid port");
    };
    Ok((host, port))
}

fn parse_host(host: &str) -> Result<Host, &'static str> {
    if let Some(inner) = host.strip_prefix('[') {
        let inner = inner.strip_suffix(']').ok_or("unterminated IPv6 bracket")?;
        if inner.contains('%') {
            return Err("IPv6 zone identifiers are not allowed");
        }
        // std parsing: rejects zone ids, non-canonical junk; accepts "::ffff:1.2.3.4" forms.
        return inner
            .parse::<Ipv6Addr>()
            .map(|a| Host::Ip(IpAddr::V6(a)))
            .map_err(|_| "invalid IPv6 address");
    }
    if host.is_empty() {
        return Err("host is empty");
    }
    // Only letters, digits, '-', '_' and '.' (punycode for IDNs). '%' escapes, ':' and everything else refused.
    if !host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("host contains characters that are not allowed");
    }
    let trimmed = host.strip_suffix('.').unwrap_or(host);
    if trimmed.is_empty() || trimmed.split('.').any(str::is_empty) || trimmed.len() > 253 {
        return Err("malformed host name");
    }
    if ends_in_a_number(trimmed) {
        return parse_ipv4_whatwg(trimmed)
            .map(|a| Host::Ip(IpAddr::V4(a)))
            .ok_or("invalid numeric host");
    }
    Ok(Host::Name(trimmed.to_ascii_lowercase()))
}

/// WHATWG "ends in a number": the last label is all decimal digits, or `0x`/`0X` followed by hex digits (or
/// nothing). Such a host MUST parse as IPv4 or be refused; it can never be a DNS name.
fn ends_in_a_number(host: &str) -> bool {
    let last = host.rsplit('.').next().unwrap_or("");
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    last.len() >= 2
        && (last.starts_with("0x") || last.starts_with("0X"))
        && last[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn parse_ipv4_number(part: &str) -> Option<u64> {
    let (digits, radix) =
        if let Some(h) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
            (h, 16)
        } else if part.len() > 1 && part.starts_with('0') {
            (&part[1..], 8)
        } else {
            (part, 10)
        };
    if digits.is_empty() {
        return Some(0); // "0x" is zero in WHATWG
    }
    u64::from_str_radix(digits, radix).ok()
}

/// WHATWG IPv4 parser: 1 to 4 parts, each decimal, octal (leading 0) or hex (0x); all parts but the last are
/// at most 255 and the last fills the remaining bytes. `None` on any failure.
fn parse_ipv4_whatwg(host: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let nums = parts
        .iter()
        .map(|p| parse_ipv4_number(p))
        .collect::<Option<Vec<u64>>>()?;
    let (last, head) = nums.split_last()?;
    if head.iter().any(|&n| n > 255) {
        return None;
    }
    let limit = 256u64.checked_pow(u32::try_from(5 - nums.len()).ok()?)?;
    if *last >= limit {
        return None;
    }
    let mut v = *last;
    for (i, n) in head.iter().enumerate() {
        v += n << (8 * (3 - i));
    }
    Some(Ipv4Addr::from(u32::try_from(v).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(u: &str) -> Result<CheckedUrl, FetchError> {
        check_url(u, &Policy::default(), Origin::Initial)
    }

    fn blocked(u: &str) -> bool {
        matches!(init(u), Err(FetchError::BlockedTarget(_)))
    }

    #[test]
    fn public_urls_pass_with_default_ports() {
        let c = init("https://Example.COM/a?b#c").unwrap();
        assert_eq!(
            c,
            CheckedUrl {
                https: true,
                host: Host::Name("example.com".into()),
                port: 443
            }
        );
        assert_eq!(init("http://example.com:8080/").unwrap().port, 8080);
        assert_eq!(
            init("HTTP://example.com.").unwrap().host,
            Host::Name("example.com".into())
        );
        assert_eq!(init("http://example.com:/").unwrap().port, 80);
        assert_eq!(
            init("http://8.8.8.8/").unwrap().host,
            Host::Ip("8.8.8.8".parse().unwrap())
        );
        assert_eq!(
            init("https://[2606:4700:4700::1111]:8443/x").unwrap(),
            CheckedUrl {
                https: true,
                host: Host::Ip("2606:4700:4700::1111".parse().unwrap()),
                port: 8443
            }
        );
        assert!(init("http://example.com?q=1").is_ok());
        assert!(init("http://example.com#f").is_ok());
    }

    #[test]
    fn only_http_and_https_schemes() {
        for u in [
            "file:///etc/passwd",
            "ftp://a.com",
            "gopher://a.com",
            "javascript:alert(1)",
            "data:text/html,x",
            "ws://a.com",
            "example.com",
            "",
            "http:",
            "http:/a.com",
            "http:a.com",
            "http:\\\\a.com",
            " http://a.com",
        ] {
            assert!(
                matches!(
                    init(u),
                    Err(FetchError::InvalidArgument { field: "url", .. })
                ),
                "{u}"
            );
        }
    }

    #[test]
    fn initial_url_errors_are_invalid_argument_naming_url_and_redirect_errors_are_blocked_target() {
        let e = init("ftp://a.com").unwrap_err();
        assert!(e.to_string().starts_with("url:"), "{e}");
        assert_eq!(e.code(), "invalid_argument");
        let e = check_url("ftp://a.com", &Policy::default(), Origin::Redirect).unwrap_err();
        assert_eq!(e.code(), "blocked_target");
    }

    #[test]
    fn userinfo_zone_ids_empty_hosts_and_bad_ports_are_refused() {
        for u in [
            "http://user@example.com/",
            "http://user:pw@example.com/",
            "http://public.example.com@127.0.0.1/",
            "http://@example.com/",
            "http://:pw@example.com/",
            "http://[fe80::1%25eth0]/",
            "http://[fe80::1%eth0]/",
            "http:///",
            "http://:80/",
            "http://example.com:0/",
            "http://example.com:65536/",
            "http://example.com:abc/",
            "http://example.com:-1/",
            "http://a..com/",
            "http://.example.com/",
            "http://exa mple.com/",
            "http://example.com\t/",
            "http://exam\nple.com/",
            "http://%31%32%37.0.0.1/",
            "http://ex%41mple.com/",
            "http://bücher.example/",
            "http://[::1/",
            "http://[::1]x/",
            "http://[::1]:80:90/",
            "http://a:b:c/",
        ] {
            assert!(init(u).is_err(), "{u:?} must be refused");
        }
    }

    #[test]
    fn userinfo_in_redirect_is_blocked_target() {
        let e = check_url("http://a@b.com/", &Policy::default(), Origin::Redirect).unwrap_err();
        assert_eq!(e.code(), "blocked_target");
    }

    #[test]
    fn ip_literals_in_every_spelling_are_blocked_before_resolution() {
        for u in [
            // canonical
            "http://127.0.0.1/",
            "http://127.0.0.1:8080/x",
            "http://10.0.0.1/",
            "http://169.254.169.254/latest/meta-data",
            "http://168.63.129.16/",
            "http://0.0.0.0/",
            "http://192.168.1.1/",
            "http://100.64.0.1/",
            "http://255.255.255.255/",
            "http://127.0.0.1./",
            // decimal, octal, hex, mixed and short forms
            "http://2130706433/",   // 127.0.0.1
            "http://017700000001/", // octal
            "http://0x7f000001/",   // hex
            "http://0X7F000001/",
            "http://0x7f.0.0.1/",
            "http://0177.0.0.1/",
            "http://127.1/",
            "http://127.0.1/",
            "http://0/", // 0.0.0.0
            "http://0x0/",
            "http://0x/",
            "http://00/",
            "http://2852039166/", // 169.254.169.254
            "http://0xa9fea9fe/",
            "http://0251.0376.0251.0376/",
            "http://169.254.43518/",
            "http://1.1.1.1.0x7f000001/", // refused as invalid numeric host (5 parts): still not dialled
            "http://10.1/",
            "http://0300.0250.0.1/", // 192.168.0.1
            "http://3232235777/",    // 192.168.1.1
            // bracketed IPv6
            "http://[::1]/",
            "http://[::1]:8080/",
            "http://[0:0:0:0:0:0:0:1]/",
            "http://[::]/",
            "http://[fe80::1]/",
            "http://[fc00::1]/",
            "http://[fd00:ec2::254]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:7f00:1]/",
            "http://[::ffff:169.254.169.254]/",
            "http://[::10.0.0.1]/",
            "http://[64:ff9b::a00:1]/",
            "http://[64:ff9b::10.0.0.1]/",
            "http://[2002:a00:1::]/",
            "http://[2002:7f00:1::1]/",
            "http://[ff02::1]/",
            "http://[2001:db8::1]/",
            "http://[2001::1]/",
            "http://[100::1]/",
            "http://[fec0::1]/",
        ] {
            assert!(init(u).is_err(), "{u} must be refused");
        }
        // The specific literals are BlockedTarget (not merely malformed) with a category and no address.
        for u in [
            "http://127.0.0.1/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://169.254.169.254/",
            "http://0251.0376.0251.0376/",
        ] {
            let e = init(u).unwrap_err();
            assert_eq!(e.code(), "blocked_target", "{u}: {e}");
            let m = e.to_string();
            assert!(
                !m.contains("127.") && !m.contains("169.") && !m.contains("::1"),
                "{u}: leaks address: {m}"
            );
        }
    }

    #[test]
    fn numeric_hosts_are_never_names() {
        // A host whose last label is numeric is IPv4 or refused, never resolved as a DNS name.
        for u in [
            "http://1.2.3.4.5/",
            "http://256.1.1.1/",
            "http://1.2.3.256/",
            "http://08.0.0.1/",
            "http://4294967296/",
            "http://0x100000000/",
            "http://999999999999999999999/",
        ] {
            let r = init(u);
            assert!(
                !matches!(
                    r,
                    Ok(CheckedUrl {
                        host: Host::Name(_),
                        ..
                    })
                ),
                "{u} became a name"
            );
        }
        // Public numeric forms fold to the address.
        assert_eq!(
            init("http://134744072/").unwrap().host,
            Host::Ip("8.8.8.8".parse().unwrap())
        );
        assert_eq!(
            init("http://0x8.8.8.8/").unwrap().host,
            Host::Ip("8.8.8.8".parse().unwrap())
        );
        assert_eq!(
            init("http://8.8.2056/").unwrap().host,
            Host::Ip("8.8.8.8".parse().unwrap())
        );
        // A name that merely contains digits is a name.
        assert_eq!(
            init("http://1password.com/").unwrap().host,
            Host::Name("1password.com".into())
        );
        assert_eq!(
            init("http://a.0x7f.example/").unwrap().host,
            Host::Name("a.0x7f.example".into())
        );
    }

    #[test]
    fn localhost_names_are_blocked_unless_loopback_is_permitted() {
        for u in [
            "http://localhost/",
            "http://LOCALHOST:3000/",
            "http://localhost./",
            "http://foo.localhost/",
        ] {
            assert!(blocked(u), "{u}");
        }
        assert!(init("http://notlocalhost.example/").is_ok());
        let p = Policy::permit_loopback_for_tests();
        assert!(check_url("http://localhost/", &p, Origin::Initial).is_ok());
        assert!(check_url("http://127.0.0.1:9/", &p, Origin::Initial).is_ok());
        assert!(check_url("http://[::1]/", &p, Origin::Initial).is_ok());
        // relaxed policy still refuses the rest
        assert!(check_url("http://10.0.0.1/", &p, Origin::Initial).is_err());
        assert!(check_url("http://169.254.169.254/", &p, Origin::Initial).is_err());
    }

    /// Property-style: every blocked v4 range's first/last address, spelled in dotted, decimal, hex, octal and
    /// IPv4-mapped bracketed form, is refused by `check_url`.
    #[test]
    fn every_blocked_v4_range_is_refused_in_every_spelling() {
        for &(net, prefix, _, cat) in ranges::V4_BLOCKED {
            let last =
                u32::try_from(u64::from(net) + (1u64 << (32 - u32::from(prefix))) - 1).unwrap();
            for n in [net, last] {
                let a = Ipv4Addr::from(n);
                let m = a.to_ipv6_mapped();
                for u in [
                    format!("http://{a}/"),
                    format!("http://{n}/"),
                    format!("http://0x{n:x}/"),
                    format!("http://0{n:o}/"),
                    format!("http://[{m}]/"),
                    format!("http://[::{}]/", a),
                ] {
                    assert!(init(&u).is_err(), "{cat}: {u} must be refused");
                }
            }
        }
    }
}
