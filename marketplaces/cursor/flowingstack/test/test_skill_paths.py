import os
import re
import tempfile
import unittest

from lib import ROOT, read

CITE = re.compile(r"(?<![\w./-])((?:bin|references)/[\w-]+(?:\.[\w-]+)*)")
LINK = re.compile(r"\]\(([^)\s]+)\)")


def skill_files(root):
    s = os.path.join(root, "skills")
    for d in sorted(os.listdir(s)):
        f = os.path.join(s, d, "SKILL.md")
        if os.path.exists(f):
            yield f


def ref_files(root):
    s = os.path.join(root, "skills")
    for d, _, fs in os.walk(s):
        if os.path.basename(d) == "references":
            yield from (os.path.join(d, n) for n in sorted(fs) if n.endswith(".md"))


def cite_ok(root, f, m):
    return any(os.path.exists(os.path.join(b, m)) for b in (os.path.dirname(f), root))


def link_ok(f, t):
    p = t.split("#")[0]
    return not p or re.match(r"[a-z]+:", p) or os.path.exists(os.path.join(os.path.dirname(f), p))


def scan(root, files, pat, ok):
    bad = []
    for f in files:
        for n, line in enumerate(read(f).splitlines(), 1):
            bad += [f"{os.path.relpath(f, root)}:{n}: {m}" for m in pat.findall(line) if not ok(f, m)]
    return bad


def bad_cites(root):
    return scan(root, skill_files(root), CITE, lambda f, m: cite_ok(root, f, m))


def bad_links(root):
    return scan(root, [*skill_files(root), *ref_files(root)], LINK, link_ok)


def fixture(t):
    d = os.path.join(t, "skills", "x", "references")
    os.makedirs(d)
    with open(os.path.join(t, "skills", "x", "SKILL.md"), "w") as f:
        f.write("run bin/gone\n[a](references/gone.md)\n")
    with open(os.path.join(d, "r.md"), "w") as f:
        f.write("[b](../nope.md)\n")


class SkillPaths(unittest.TestCase):
    def test_cited_bin_and_references_exist(self):
        self.assertEqual(bad_cites(ROOT), [])

    def test_relative_links_resolve(self):
        self.assertEqual(bad_links(ROOT), [])

    def test_broken_fixture_fails(self):
        with tempfile.TemporaryDirectory() as t:
            fixture(t)
            self.assertEqual(bad_cites(t), ["skills/x/SKILL.md:1: bin/gone", "skills/x/SKILL.md:2: references/gone.md"])
            self.assertEqual(len(bad_links(t)), 2)


if __name__ == "__main__":
    unittest.main()
