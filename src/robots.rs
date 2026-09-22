//! robots.txt parsing and matching (B-4). A mechanism only, and inert by default: it is fully implemented and
//! tested here, but `crate::fetch::FetchClient` only calls it when `Config.robots_txt` is
//! [`crate::config::RobotsMode::Enforce`], which nothing sets by default -- the default stays
//! [`crate::config::RobotsMode::Ignore`] (`FETCH_ROBOTS_TXT=enforce` to opt in), and this module does not decide
//! or touch that default. This mirrors C-2's private-host allowlist (`src/config.rs`, `Policy::for_build`): a
//! fully working, fully tested mechanism, switched off until an operator opts in. OQ-3 is RESOLVED (2026-09-22): the
//! default stays `ignore` by design. OQ-3's original framing -- whether fetches
//! should respect robots.txt BY DEFAULT -- is a still-open product-owner decision; this work answers only "how
//! would enforcement work", never "should it be on".
//!
//! Matching follows the de facto standard algorithm (as used by Google's robots.txt parser and described in
//! RFC 9309 section 2.2.1), simplified to the common cases: the most specific matching `User-agent` group
//! applies (the longest product token that is a case-insensitive prefix of our own user agent, falling back to
//! a `*` group when present); within that group, the longest matching `Allow`/`Disallow` path prefix wins, and
//! an `Allow` beats a `Disallow` of equal length. A path with no matching rule, a file with no applicable
//! group, and an empty or entirely unparseable file all allow everything (fail-open, per the AC: a missing or
//! broken robots.txt must not block the fetch). Pure logic, no I/O: `crate::fetch::mod` fetches the robots.txt
//! body through the existing guarded client (SSRF checks, redirects, size cap) and hands the text here.
//!
//! Simplification (finding #7, round-2 review): rule paths are matched as plain, literal prefixes -- there is
//! no support for the `*` (wildcard) or `$` (end-of-path anchor) path-matching operators from RFC 9309 section
//! 2.2.3, and matching is against `path` only (no query string). A rule using either operator, or one intended
//! to apply only with a particular query string, is therefore matched more broadly than a fully RFC-9309-compliant
//! parser would: it still participates in the longest-literal-prefix comparison above, verbatim, wildcard
//! characters included. This fails open (matches nothing it should not have generally *disallowed*, but also
//! nothing more specific than a broader rule requires), consistent with the module's overall fail-open stance.

/// One `User-agent:` record: the (lower-cased) product tokens it applies to, and its `Allow`/`Disallow` rules
/// in file order, each a path prefix and whether it allows (`true`) or disallows (`false`) that prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    agents: Vec<String>,
    rules: Vec<(String, bool)>,
}

/// Whether `path` (the request's path, e.g. `/private/x`; an empty path is treated as `/`) is allowed by
/// `robots_txt` for `user_agent`. Malformed or unrecognized lines are ignored rather than failing the parse;
/// a file with no rule matching `path` in the applicable group, or no applicable group at all, allows it.
#[must_use]
pub fn is_allowed(robots_txt: &str, user_agent: &str, path: &str) -> bool {
    let path = if path.is_empty() { "/" } else { path };
    let records = parse(robots_txt);
    let Some(record) = select_record(&records, user_agent) else {
        return true;
    };
    let mut best_len: Option<usize> = None;
    let mut best_allow = true;
    for (prefix, allow) in &record.rules {
        if !path.starts_with(prefix.as_str()) {
            continue;
        }
        let len = prefix.len();
        let takes_it = match best_len {
            None => true,
            Some(blen) if len > blen => true,
            Some(blen) if len == blen && *allow && !best_allow => true, // Allow wins a tie (RFC 9309 2.2.1)
            _ => false,
        };
        if takes_it {
            best_len = Some(len);
            best_allow = *allow;
        }
    }
    best_allow
}

/// Parses `User-agent:` / `Allow:` / `Disallow:` records (RFC 9309 section 2.2). Anything else (`Sitemap:`,
/// `Crawl-delay:`, unknown fields, lines with no `:`, blank lines, `#` comments) is ignored. A `#` starts a
/// comment anywhere on a line, even mid-value. One or more consecutive `User-agent:` lines that are then
/// followed by `Allow`/`Disallow` lines form one record covering all of those agents; a `User-agent:` line
/// seen again after rules have started closes the current record and opens a new one.
fn parse(text: &str) -> Vec<Record> {
    let mut records = Vec::new();
    let mut agents: Vec<String> = Vec::new();
    let mut rules: Vec<(String, bool)> = Vec::new();
    let mut rules_started = false;

    for raw_line in text.lines() {
        let line = match raw_line.split_once('#') {
            Some((before, _)) => before,
            None => raw_line,
        }
        .trim();
        if line.is_empty() {
            continue;
        }
        let Some((field, value)) = line.split_once(':') else {
            continue; // malformed line, ignored
        };
        let value = value.trim();
        match field.trim().to_ascii_lowercase().as_str() {
            "user-agent" => {
                if value.is_empty() {
                    continue;
                }
                if rules_started {
                    records.push(Record {
                        agents: std::mem::take(&mut agents),
                        rules: std::mem::take(&mut rules),
                    });
                    rules_started = false;
                }
                agents.push(value.to_ascii_lowercase());
            }
            "disallow" => {
                rules_started = true;
                // An empty Disallow value is the documented "allow everything" spelling.
                rules.push((value.to_string(), value.is_empty()));
            }
            "allow" => {
                rules_started = true;
                if !value.is_empty() {
                    rules.push((value.to_string(), true));
                }
            }
            _ => {}
        }
    }
    if !agents.is_empty() {
        records.push(Record { agents, rules });
    }
    records
}

/// The most specific matching record for `user_agent`: the longest (lower-cased) agent token in any record
/// that is a prefix of `user_agent` (case-insensitive), falling back to a `*` record when present, else `None`.
fn select_record<'a>(records: &'a [Record], user_agent: &str) -> Option<&'a Record> {
    let ua = user_agent.to_ascii_lowercase();
    let mut best: Option<(usize, &Record)> = None;
    let mut wildcard: Option<&Record> = None;
    for record in records {
        for agent in &record.agents {
            if agent == "*" {
                wildcard = wildcard.or(Some(record));
            } else if ua.starts_with(agent.as_str())
                && best.is_none_or(|(blen, _)| agent.len() > blen)
            {
                best = Some((agent.len(), record));
            }
        }
    }
    best.map(|(_, r)| r).or(wildcard)
}

#[cfg(test)]
mod tests {
    use super::is_allowed;

    const UA: &str = "fetch-mcp/1.2.3";

    #[test]
    fn empty_robots_txt_allows_everything() {
        assert!(is_allowed("", UA, "/anything"));
        assert!(is_allowed("   \n\n  ", UA, "/private/x"));
    }

    #[test]
    fn disallow_a_prefix_for_the_wildcard_group() {
        let txt = "User-agent: *\nDisallow: /private\n";
        assert!(!is_allowed(txt, UA, "/private"));
        assert!(!is_allowed(txt, UA, "/private/x"));
        assert!(is_allowed(txt, UA, "/public"));
        assert!(is_allowed(txt, UA, "/"));
    }

    #[test]
    fn allow_overrides_a_shorter_disallow() {
        let txt = "User-agent: *\nDisallow: /private\nAllow: /private/public\n";
        assert!(!is_allowed(txt, UA, "/private/secret"));
        assert!(is_allowed(txt, UA, "/private/public"));
        assert!(is_allowed(txt, UA, "/private/public/x"));
    }

    #[test]
    fn longest_prefix_wins_regardless_of_rule_order() {
        // Allow appears before the more specific Disallow: the longer match still wins.
        let txt = "User-agent: *\nAllow: /a\nDisallow: /a/b\n";
        assert!(is_allowed(txt, UA, "/a/x"));
        assert!(!is_allowed(txt, UA, "/a/b/x"));
    }

    #[test]
    fn equal_length_allow_and_disallow_tie_to_allow() {
        let txt = "User-agent: *\nDisallow: /x\nAllow: /x\n";
        assert!(is_allowed(txt, UA, "/x"));
        let txt = "User-agent: *\nAllow: /x\nDisallow: /x\n";
        assert!(is_allowed(txt, UA, "/x"));
    }

    #[test]
    fn empty_disallow_value_means_allow_everything() {
        let txt = "User-agent: *\nDisallow:\n";
        assert!(is_allowed(txt, UA, "/private"));
    }

    #[test]
    fn most_specific_user_agent_group_wins_over_wildcard() {
        let txt = "User-agent: *\nDisallow: /\n\nUser-agent: fetch-mcp\nDisallow: /only-this\n";
        // the specific group applies, not the wildcard's blanket disallow
        assert!(is_allowed(txt, UA, "/public"));
        assert!(!is_allowed(txt, UA, "/only-this"));
    }

    #[test]
    fn a_group_with_no_matching_user_agent_and_no_wildcard_allows_everything() {
        let txt = "User-agent: somebot\nDisallow: /\n";
        assert!(is_allowed(txt, UA, "/anything"));
    }

    #[test]
    fn multiple_user_agent_lines_share_one_record() {
        let txt = "User-agent: somebot\nUser-agent: fetch-mcp\nDisallow: /shared\n";
        assert!(!is_allowed(txt, UA, "/shared"));
    }

    #[test]
    fn a_new_user_agent_after_rules_starts_a_fresh_record() {
        let txt =
            "User-agent: fetch-mcp\nDisallow: /only-here\nUser-agent: *\nDisallow: /everything\n";
        assert!(!is_allowed(txt, UA, "/only-here"));
        assert!(is_allowed(txt, UA, "/everything")); // the fetch-mcp record applies, not the later wildcard one
    }

    #[test]
    fn comments_and_malformed_lines_are_ignored_not_fatal() {
        let txt = "# a full-line comment\nUser-agent: *\nDisallow: /private # trailing comment\nnotarule\n: novalue\n";
        assert!(!is_allowed(txt, UA, "/private/x"));
        assert!(is_allowed(txt, UA, "/public"));
    }

    #[test]
    fn matching_is_case_insensitive_for_the_user_agent_token() {
        let txt = "User-agent: Fetch-MCP\nDisallow: /x\n";
        assert!(!is_allowed(txt, UA, "/x"));
    }

    #[test]
    fn an_empty_path_is_treated_as_root() {
        let txt = "User-agent: *\nDisallow: /\n";
        assert!(!is_allowed(txt, UA, ""));
    }

    /// Finding #7 (round-2 review): no `*`/`$` wildcard support (RFC 9309 2.2.3) and no query-string matching --
    /// documented as fail-open in the module docs. This pins the current (simplified) behaviour: a `*` in a
    /// rule is matched literally, so it disallows only paths starting with that literal text, not the intended
    /// wildcard pattern, and a rule is compared against the path alone, ignoring any query string.
    #[test]
    fn wildcard_and_query_string_rules_fail_open_rather_than_pattern_matching() {
        let txt = "User-agent: *\nDisallow: /private/*.pdf$\n";
        // Intended (RFC 9309 wildcard semantics) to disallow this; the literal-prefix simplification does not.
        assert!(is_allowed(txt, UA, "/private/report.pdf"));
        // The literal rule text itself, taken as a plain prefix, still disallows.
        assert!(!is_allowed(txt, UA, "/private/*.pdf$/x"));

        let txt = "User-agent: *\nDisallow: /search?blocked=1\n";
        // A query string on the request path is not considered: the same path prefix without the query is
        // already covered by the literal rule text, so this is disallowed regardless of the query given.
        assert!(!is_allowed(txt, UA, "/search?blocked=1"));
        assert!(is_allowed(txt, UA, "/search?other=1")); // different literal prefix: allowed
    }

    /// Finding #12 (round-2 review): `Crawl-delay:` is a recognized field (RFC 9309 is silent on it; it is a
    /// de facto extension) but this module has no rate-limiting concept, so it must be parsed-and-ignored, not
    /// mistaken for an `Allow`/`Disallow` rule (which would corrupt matching) or for a group-closing line.
    #[test]
    fn crawl_delay_lines_are_parsed_and_ignored_not_mistaken_for_a_rule() {
        let txt = "User-agent: *\nCrawl-delay: 10\nDisallow: /private\n";
        assert!(!is_allowed(txt, UA, "/private"));
        assert!(is_allowed(txt, UA, "/public"));
        // Between two User-agent lines, Crawl-delay does not itself start (or close) a rules-bearing record.
        let txt = "User-agent: *\nUser-agent: fetch-mcp\nCrawl-delay: 10\nDisallow: /x\n";
        assert!(!is_allowed(txt, UA, "/x"));
    }
}
