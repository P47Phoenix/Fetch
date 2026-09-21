//! Hard memory bound on hostile HTML (fix-pass 1, architect B2). This file is its own test binary with a single
//! test, so `VmHWM` (the process's peak RSS) belongs to nothing else. The converter is driven exactly as the fetch
//! path drives it (16 KiB slices), three at a time (`FETCH_MAX_CONCURRENCY`). The assertion is on RSS, not only on
//! the result: before the attribute guard one tag of 1.6 MiB of `a a a` cost ~114 MB per fetch.
#![cfg(target_os = "linux")]

use fetch_mcp::convert::markdown::MarkdownConverter;
use fetch_mcp::convert::{ConvertError, Converter};

const MIB: usize = 1024 * 1024;
const SLICE: usize = 16 * 1024;

fn hwm_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmHWM:"))
                .and_then(|v| v.split_whitespace().next().and_then(|n| n.parse().ok()))
        })
        .expect("VmHWM in /proc/self/status")
}

fn convert(html: &str) -> Result<usize, ConvertError> {
    let mut c = MarkdownConverter::new(None);
    let mut out = 0usize;
    for part in html.as_bytes().chunks(SLICE) {
        let s = String::from_utf8_lossy(part);
        c.push(&s, &mut |t| out += t.len())?;
    }
    c.finish(&mut |t| out += t.len())?;
    Ok(out)
}

/// (name, fixture builder, must the converter refuse it)
type Scenario = (&'static str, Box<dyn Fn() -> String>, bool);

fn scenarios() -> Vec<Scenario> {
    vec![
        (
            "attr-bomb-1.9MiB",
            Box::new(|| format!("<div {}>x</div>", "a ".repeat(950 * 1024))),
            true,
        ),
        (
            "attr-bomb-slash-1.9MiB",
            Box::new(|| format!("<div {}>x</div>", "a/".repeat(950 * 1024))),
            true,
        ),
        (
            "attr-bomb-valued-1.9MiB",
            Box::new(|| format!("<div {}>x</div>", "a=1 ".repeat(480 * 1024))),
            true,
        ),
        (
            "attr-bomb-after-quoted-gt",
            Box::new(|| format!("<div x=\">\" {}>x</div>", "a ".repeat(950 * 1024))),
            true,
        ),
        (
            "attr-bomb-hidden-in-script-context",
            Box::new(|| {
                format!(
                    "<script>a<b \"</script><div x=\">\" {}>x</div>",
                    "a ".repeat(950 * 1024)
                )
            }),
            true,
        ),
        (
            "unterminated-attr-bomb",
            Box::new(|| format!("<div {}", "a ".repeat(950 * 1024))),
            true,
        ),
        (
            // one attribute of 1.9 MiB: allowed (a few attributes are cheap), bounded by the 2 MiB limit
            "one-huge-attribute-1.9MiB",
            Box::new(|| format!("<img alt=\"{}\">x", "z".repeat(1900 * 1024))),
            false,
        ),
        (
            // just under the per-tag attribute cap, 2000 times: each tag is released before the next
            "many-tags-at-the-attr-cap",
            Box::new(|| ("<i ".to_string() + &"a ".repeat(1000) + "></i>").repeat(900)),
            false,
        ),
        (
            "thousands-of-small-tags",
            Box::new(|| "<b x=1 y=2>t</b>".repeat(120_000)),
            false,
        ),
    ]
}

#[test]
fn hostile_pages_stay_within_the_memory_budget_three_at_a_time() {
    // Warm up the allocator and the harness so the baseline is honest.
    assert!(convert("<p>warm</p>").is_ok());
    let mut worst_delta = 0usize;
    for (name, make, must_refuse) in scenarios() {
        // Build the fixture, then reset the peak counter (`clear_refs` 5) so the bound is on what the conversion
        // costs, not on the test's own fixture. The absolute check below still covers the whole process.
        let html = make();
        // (If the reset is refused the delta below can only read low, but the absolute 40 MiB check still holds.)
        let _ = std::fs::write("/proc/self/clear_refs", "5");
        let base = hwm_kib();
        let html = std::sync::Arc::new(html);
        let handles: Vec<_> = (0..3)
            .map(|_| {
                let h = std::sync::Arc::clone(&html);
                std::thread::spawn(move || convert(&h))
            })
            .collect();
        for h in handles {
            let r = h.join().expect("no panic");
            if must_refuse {
                assert_eq!(r, Err(ConvertError::Limit), "{name}: must fail closed");
            } else {
                assert!(r.is_ok(), "{name}: legitimate shape must convert: {r:?}");
            }
        }
        let after = hwm_kib();
        worst_delta = worst_delta.max(after.saturating_sub(base));
        eprintln!(
            "hostile-rss {name}: peak {} MiB (+{} MiB over baseline {} MiB), input {:.1} MiB x3",
            after / 1024,
            after.saturating_sub(base) / 1024,
            base / 1024,
            html.len() as f64 / MIB as f64
        );
        // Hard bounds: the whole process (harness + 3 concurrent conversions + 3 copies of the input's slices)
        // under the 40 MiB product target, and never more than 24 MiB above the idle baseline.
        assert!(
            after / 1024 <= 40,
            "{name}: peak RSS {} MiB > 40 MiB",
            after / 1024
        );
        assert!(
            after.saturating_sub(base) / 1024 <= 24,
            "{name}: +{} MiB over baseline",
            after.saturating_sub(base) / 1024
        );
    }
    eprintln!(
        "hostile-rss worst above baseline: {} MiB",
        worst_delta / 1024
    );
}
