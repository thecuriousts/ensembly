import sys
from collections import Counter


def load(path):
    base = {}
    for n, l in enumerate(open(path, encoding="utf-8"), 1):
        c = l.rstrip("\n").split("\t")
        if not l.strip():
            continue
        if len(c) != 3 or not c[2].isdigit():
            print(f"{path}:{n}: baseline row needs path, rule, count", file=sys.stderr)
            sys.exit(2)
        base[(c[0], c[1])] = int(c[2])
    return base


def live(keys, base):
    n = Counter(keys)
    return [k not in base or n[k] > base[k] for k in keys]


def rows(keys):
    return [f"{p}\t{r}\t{c}\n" for (p, r), c in sorted(Counter(keys).items())]
