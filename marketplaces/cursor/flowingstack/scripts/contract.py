import re

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
