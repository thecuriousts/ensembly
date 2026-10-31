import os
import re
import unittest

from lib import ROOT, read

MENTION = re.compile(r"(?<![\w./-])((?:tstack/|intelli-route/)?(?:bin|references)/[\w-]+(?:\.[\w-]+)*(?:/[\w-]+(?:\.[\w-]+)*)*)")
ALIAS = {"tstack/": "", "intelli-route/": "skills/intelli-route/"}


def docs():
    for d, _, fs in os.walk(os.path.join(ROOT, "skills")):
        for n in fs:
            if n.endswith(".md"):
                yield os.path.join(d, n)


def peers():
    rows = {}
    p = os.path.join(ROOT, "references", "peers.md")
    for l in (read(p) if os.path.exists(p) else "").splitlines():
        c = [x.strip().strip("`") for x in l.strip().strip("|").split("|")]
        if len(c) == 3 and c[0] not in ("file", "---"):
            rows[(c[0], c[1])] = c[2]
    return rows


def resolves(f, m):
    for a, to in ALIAS.items():
        if m.startswith(a):
            return os.path.exists(os.path.join(ROOT, to, m[len(a):]))
    d = os.path.dirname(f)
    while True:
        if os.path.exists(os.path.join(d, m)):
            return True
        if os.path.samefile(d, ROOT):
            return False
        d = os.path.dirname(d)


class Refs(unittest.TestCase):
    def test_every_bin_and_references_mention_ships_or_has_a_peer_row(self):
        gaps = peers()
        bad = []
        for f in docs():
            r = os.path.relpath(f, ROOT)
            for n, line in enumerate(read(f).splitlines(), 1):
                for m in MENTION.findall(line):
                    if not resolves(f, m) and not gaps.get((r, m)):
                        bad.append(f"{r}:{n}: {m}")
        self.assertEqual(bad, [])

    def test_peer_rows_name_a_live_gap(self):
        for (f, m), todo in peers().items():
            with self.subTest(m):
                self.assertFalse(resolves(os.path.join(ROOT, f), m))
                self.assertIn(m, read(os.path.join(ROOT, f)))
                self.assertTrue(todo)


if __name__ == "__main__":
    unittest.main()
