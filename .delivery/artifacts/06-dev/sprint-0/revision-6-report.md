# Revision 6: portable gzip fixture verification

Cause: gzip bytes depend on the zlib build; the GitHub runner produced 783562 bytes vs committed 776917. The selftest then hit StopIteration because later phases ran without valid fixtures.

Changes
- bench/manifest.json (version 2): html_5mib_gz pins decompressed_size and decompressed_sha256 (equal to html_5mib), plus compressed_size_min/max (256 KiB..2 MiB) and a reference size. Non-gz fixtures keep strict size+sha256.
- bench/fixtures.py: verify() decompresses the gz and compares size and sha256, and bounds the wire size. New resolve() sets the gz m["size"] to the real on-disk size, so serve.py Content-Length and the g4a-5mib-gz byte floor use actual wire bytes; the 5 MiB cap still applies to the decompressed bytes.
- bench/measure.py: calls resolve(); fixture_sha256 uses the decompressed hash for gz; refusal text updated.
- bench/selftest.py: exits with a clean FAIL line if fixtures do not verify.
- docs/BENCHMARK.md: fixture text updated.
- Nothing else (selftest, ci.yml) depends on gz byte identity.

Proof: gzip -1 (1113280 B), gzip -9 (758745 B) and zlib level 3/memLevel 1 all verify OK; a bit-flipped payload inside a valid gzip and a modified plain fixture are REFUSED (exit 2).

Clean git archive HEAD: fmt, clippy (default + bench-loopback), cargo test x3, bench/selftest.py PASSED, guard and guard --self-test OK. actionlint clean in repo.
