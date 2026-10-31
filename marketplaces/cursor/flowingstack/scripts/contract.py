import re
import sys

from rootpath import plugin_root, rel

KINDS = {"test": "bug", "acceptance": "feature", "unchanged": "refactor", "metric": "perf", "heldout": "eval"}


def parse(text):
    out = {}
    for l in text.splitlines():
        if ":" in l and not l.startswith((" ", "#")):
            k, v = l.split(":", 1)
            out[k.strip()] = v.strip()
    return out


def route(c):
    kind, _, rest = c.get("check", "").partition(":")
    r = KINDS.get(kind.strip())
    if r is None or not rest.strip() or (r == "perf" and not re.search(r"\d", rest)):
        return "empty"
    return r


def main(argv):
    if len(argv) != 1:
        print("usage: contract-check FILE", file=sys.stderr)
        return 1
    c = parse(open(rel(plugin_root(), argv[0]), encoding="utf-8").read())
    if not all(c.get(k) for k in ("id", "input", "output", "counter")):
        print("refused: id, input, output, counter required", file=sys.stderr)
        return 2
    r = route(c)
    print(f"id={c['id']} route={r}")
    return 1 if r == "empty" else 0
