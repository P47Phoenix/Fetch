#!/usr/bin/env python3
"""Fixture HTTP server with a server-side counter of body bytes handed to the socket per route.

Routes: /5mb.html, /5mb.html.gz (Content-Encoding: gzip), /50mb-cl.html (Content-Length),
/50mb-chunked.html (chunked, no Content-Length), /slow (slow drip). Loopback only.
The counter is an upper bound on what the client consumed (kernel buffers); it backs the
valid-run rule (early-stop detection) in docs/BENCHMARK.md section 6.
"""
import http.server, os, sys, threading, time

ROUTES = {  # path -> (fixture name, mode, extra headers)
    "/5mb.html": ("html_5mib", "cl", {}),
    "/5mb.html.gz": ("html_5mib_gz", "cl", {"Content-Encoding": "gzip"}),
    "/50mb-cl.html": ("html_50mib", "cl", {}),
    "/50mb-chunked.html": ("html_50mib", "chunked", {}),
    "/slow": (None, "slow", {}),
}


class FixtureServer:
    def __init__(self, fixture_dir, manifest, port=0):
        self.counters, self._lock = {}, threading.Lock()
        outer = self

        class H(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"
            def log_message(self, *a): pass
            def do_GET(self):
                route = ROUTES.get(self.path.split("?")[0])
                if not route:
                    self.send_error(404); return
                name, mode, extra = route
                self.close_connection = True
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
                            outer._send(self, path, b"x" * 64, True); time.sleep(1)
                    else:
                        with open(os.path.join(fixture_dir, m["file"]), "rb") as f:
                            for blk in iter(lambda: f.read(64 * 1024), b""):
                                outer._send(self, path, blk, mode == "chunked")
                    if mode != "cl":
                        self.wfile.write(b"0\r\n\r\n")
                except (BrokenPipeError, ConnectionResetError):
                    pass

        self.httpd = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
        self.httpd.daemon_threads = True
        self.port = self.httpd.server_address[1]

    def _send(self, h, path, data, chunked):
        h.wfile.write(b"%x\r\n%s\r\n" % (len(data), data) if chunked else data)
        h.wfile.flush()
        with self._lock:
            self.counters[path] = self.counters.get(path, 0) + len(data)

    def bytes_sent(self, path):
        with self._lock: return self.counters.get(path, 0)
    def reset(self):
        with self._lock: self.counters.clear()
    def start(self):
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start(); return self
    def stop(self):
        self.httpd.shutdown(); self.httpd.server_close()


if __name__ == "__main__":
    import fixtures
    d = sys.argv[2] if len(sys.argv) > 2 else os.path.join(fixtures.HERE, "fixtures")
    bad = fixtures.verify(d)
    if bad: sys.exit("fixture mismatch: " + "; ".join(bad))
    s = FixtureServer(d, fixtures.load_manifest(), int(sys.argv[1]) if len(sys.argv) > 1 else 8080)
    print("serving on 127.0.0.1:%d" % s.port, flush=True); s.httpd.serve_forever()
