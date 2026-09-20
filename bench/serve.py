#!/usr/bin/env python3
"""Fixture HTTP server with a server-side counter of body bytes handed to the socket per route.

Routes: /5mb.html, /5mb.html.gz (Content-Encoding: gzip), /50mb-cl.html (Content-Length),
/50mb-chunked.html (chunked, no Content-Length), /late-landmark.html, /slow (slow drip), and
/redir/N (N >= 1: 302 with a REDIR_BODY-byte body to /redir/N-1; /redir/0: 200 with REDIR_FINAL bytes; all counted under "/redir"). Loopback only.
The counter (incremented just before each write) is an upper bound on what the client consumed (kernel buffers); it backs the
valid-run rule (early-stop detection) in docs/BENCHMARK.md section 6.
"""
import http.server, os, sys, threading, time
import fixtures

ROUTES = {  # path -> (fixture name, mode, extra headers)
    "/5mb.html": ("html_5mib", "cl", {}),
    "/5mb.html.gz": ("html_5mib_gz", "cl", {"Content-Encoding": "gzip"}),
    "/50mb-cl.html": ("html_50mib", "cl", {}),
    "/50mb-chunked.html": ("html_50mib", "chunked", {}),
    "/late-landmark.html": ("late_landmark", "cl", {}),
    "/slow": (None, "slow", {}),
}


class FixtureServer:
    def __init__(self, fixture_dir, manifest, port=0):
        self.counters, self.first, self._lock, self._gen = {}, {}, threading.Lock(), 0
        outer = self

        class H(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"
            def log_message(self, *a): pass
            def _redir(self, gen):
                try: n = int(self.path[len("/redir/"):].split("?")[0])
                except ValueError: self.send_error(404); return
                self.close_connection = True
                final = n <= 0
                body = b"r" * (fixtures.REDIR_FINAL if final else fixtures.REDIR_BODY)
                self.send_response(200 if final else 302)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                if not final: self.send_header("Location", f"/redir/{n - 1}")
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Connection", "close")
                self.end_headers()
                try: outer._send(self, "/redir", body, False, gen)
                except (BrokenPipeError, ConnectionResetError): pass

            def do_GET(self):
                route = ROUTES.get(self.path.split("?")[0])
                if self.path.startswith("/redir/"):
                    self._redir(outer._gen); return
                if not route:
                    self.send_error(404); return
                name, mode, extra = route
                self.close_connection = True
                gen = outer._gen   # a straggler from a previous sample must not count into the next one
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                for k, v in extra.items(): self.send_header(k, v)
                m = manifest["fixtures"].get(name) if name else None
                if mode == "cl":
                    self.send_header("Content-Length", str(m["size"]))
                else:
                    self.send_header("Transfer-Encoding", "chunked")
                self.send_header("Connection", "close")
                self.end_headers()
                path = self.path.split("?")[0]
                try:
                    if mode == "slow":
                        for _ in range(120):
                            outer._send(self, path, b"x" * 64, True, gen); time.sleep(1)
                    else:
                        with open(os.path.join(fixture_dir, m["file"]), "rb") as f:
                            for blk in iter(lambda: f.read(64 * 1024), b""):
                                outer._send(self, path, blk, mode == "chunked", gen)
                    if mode != "cl":
                        self.wfile.write(b"0\r\n\r\n")
                except (BrokenPipeError, ConnectionResetError):
                    pass

        self.httpd = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
        self.httpd.daemon_threads = True
        self.port = self.httpd.server_address[1]

    def _send(self, h, path, data, chunked, gen):
        with self._lock:   # count before the write: a client that aborts mid-write must not race the counter to zero
            if gen == self._gen:
                self.counters[path] = self.counters.get(path, 0) + len(data)
                self.first.setdefault(path, time.monotonic())   # first body write of this sample (timing only, never gates)
        h.wfile.write(b"%x\r\n%s\r\n" % (len(data), data) if chunked else data)
        h.wfile.flush()

    def bytes_sent(self, path):
        with self._lock: return self.counters.get(path, 0)
    def first_byte_at(self, path):
        """time.monotonic() of the first body write for path since reset(), or None (e.g. aborted on the headers)."""
        with self._lock: return self.first.get(path)
    def reset(self):
        with self._lock: self.counters.clear(); self.first.clear(); self._gen += 1
    def start(self):
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start(); return self
    def stop(self):
        self.httpd.shutdown(); self.httpd.server_close()


if __name__ == "__main__":
    d = sys.argv[2] if len(sys.argv) > 2 else os.path.join(fixtures.HERE, "fixtures")
    bad = fixtures.verify(d)
    if bad: sys.exit("fixture mismatch: " + "; ".join(bad))
    s = FixtureServer(d, fixtures.load_manifest(), int(sys.argv[1]) if len(sys.argv) > 1 else 8080)
    print("serving on 127.0.0.1:%d" % s.port, flush=True); s.httpd.serve_forever()
