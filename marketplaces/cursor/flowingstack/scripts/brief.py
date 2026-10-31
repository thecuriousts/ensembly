import re
import sys

RULES = {
    "skills": lambda t: re.search(r"(?im)^.*\bskills?\b[^\n:]*:[ \t]*\S", t),
    "snapshot": lambda t: re.search(r"(?i)\bsnapshot\b", t),
    "pr-body": lambda t: any(int(n) <= 120 for n in re.findall(r"(?i)\bPR bod(?:y|ies)\b[^\n]*?\b(\d+) words", t)),
}


def missing(text):
    return [k for k, ok in RULES.items() if not ok(text)]


def main(argv):
    if len(argv) != 1:
        print("usage: brief-lint FILE", file=sys.stderr)
        return 2
    with open(argv[0], encoding="utf-8") as f:
        m = missing(f.read())
    print("brief missing: " + " ".join(m) if m else "brief ok")
    return 1 if m else 0
