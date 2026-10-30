import os
import sys


def plugin_root(arg=None):
    return os.path.abspath(arg or os.environ.get("FLOWINGSTACK_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def inside(root, p):
    r = os.path.realpath(root)
    if os.path.isabs(p) and os.path.commonpath([r, os.path.realpath(p)]) == r:
        return os.path.relpath(os.path.realpath(p), r)
    return p


def rel(root, p):
    p = inside(root, p)
    full = os.path.join(root, p)
    r = os.path.realpath(root)
    if os.path.isabs(p) or ".." in p.split("/") or os.path.commonpath([r, os.path.realpath(full)]) != r:
        print(f"{p}: path must be relative to plugin root, no ..", file=sys.stderr)
        sys.exit(2)
    return full


def walk(root, ignore_file):
    import fnmatch
    pats = []
    f = os.path.join(root, ignore_file)
    if os.path.exists(f):
        pats = [l.strip() for l in open(f) if l.strip()]
    for d, dirs, files in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x != ".git")
        for n in sorted(files):
            r = os.path.relpath(os.path.join(d, n), root)
            if not any(fnmatch.fnmatch(r, p) for p in pats):
                yield r
