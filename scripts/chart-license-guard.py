#!/usr/bin/env python3
"""Fail if a banned chart skill's output or assets are in the repo.

Minicharts (MIT) is allowed and vendored for the E5 chart. Its sister drawing skill's
free licence bans product use, so nothing from it may ship. This file spells that
skill's name in split parts so it does not match itself.

    python3 -I scripts/chart-license-guard.py [--root DIR]   exit 0 clean, 1 hits
"""
import os
import subprocess
import sys

NEEDLE = "tiny" + "charts"
HOST = "april-zhu.com/" + NEEDLE
SELF = "scripts/chart-license-guard.py"
TEXT_EXT = {
    ".md", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".rs", ".json", ".jsonl", ".yml", ".yaml",
    ".sh", ".toml", ".txt", ".html", ".htm", ".css", ".py", ".svg", "",
}


def files(root):
    out = subprocess.run(["git", "ls-files", "-z", "-c", "-o", "--exclude-standard"],
                         cwd=root, capture_output=True, check=False)
    if out.returncode != 0:
        sys.stderr.write(out.stderr.decode() or "git ls-files failed\n")
        sys.exit(2)
    return [p for p in out.stdout.decode().split("\0") if p]


def main(argv):
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if argv[1:2] == ["--root"] and len(argv) == 3:
        root = os.path.abspath(argv[2])
    elif argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    hits = []
    listed = files(root)
    for rel in listed:
        if rel == SELF:
            continue
        base = os.path.basename(rel).lower()
        if NEEDLE in base or (base.endswith(".skill") and "tiny" in base):
            hits.append(rel + ": file name")
            continue
        if os.path.splitext(rel)[1].lower() not in TEXT_EXT:
            continue
        try:
            data = open(os.path.join(root, rel), "rb").read()
        except OSError:
            continue
        if b"\0" in data:
            continue
        text = data.decode("utf-8", "replace").lower()
        if NEEDLE in text or HOST in text:
            hits.append(rel + ": content")
    for h in hits:
        print(h)
    print("files %d hits %d" % (len(listed), len(hits)))
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
