# Generates a ~5 MB realistic-ish HTML page (deterministic).
import random
random.seed(1)
words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua".split()
def p(n): return " ".join(random.choice(words) for _ in range(n))
out = ["<!doctype html><html><head><meta charset=utf-8><title>Fixture</title><style>body{font:14px sans-serif}</style><script>var x=1;</script></head><body><nav><a href='/'>Home</a></nav><main>"]
size = 0; i = 0
while size < 5 * 1024 * 1024:
    s = (f"<section class='c{i}'><h2>Heading {i}</h2><p>{p(60)} <a href='https://example.com/{i}'>link {i}</a> <em>{p(5)}</em> <strong>{p(4)}</strong></p>"
         f"<ul><li>{p(8)}</li><li>{p(8)}</li><li><code>{p(3)}</code></li></ul>"
         f"<div><div><span>{p(12)}</span></div></div></section>\n")
    out.append(s); size += len(s); i += 1
out.append("</main></body></html>")
open("fixture5mb.html", "w").write("".join(out))
