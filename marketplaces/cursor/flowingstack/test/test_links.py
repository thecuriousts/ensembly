import os
import re
import unittest

from lib import ROOT, read

LINK = re.compile(r"\]\(([^)\s]+)\)")


def broken(top):
    bad = []
    for d, _, fs in os.walk(os.path.join(ROOT, top)):
        for n in fs:
            if not n.endswith(".md"):
                continue
            f = os.path.join(d, n)
            for i, line in enumerate(read(f).splitlines(), 1):
                for t in LINK.findall(re.sub(r"`[^`]*`", "", line)):
                    p = t.split("#")[0]
                    if p and not re.match(r"[a-z]+:", p) and not os.path.exists(os.path.join(d, p)):
                        bad.append(f"{os.path.relpath(f, ROOT)}:{i}: {t}")
    return bad


class IntelliRoute(unittest.TestCase):
    def test_skill_file(self):
        s = read(os.path.join(ROOT, "skills", "intelli-route", "SKILL.md"))
        for h in ("name: intelli-route", "description: Use when", "owner:", "source:", "last-reviewed:"):
            self.assertIn(h, s)

    def test_links_resolve(self):
        self.assertEqual(broken(os.path.join("skills", "intelli-route")), [])



class Plugin(unittest.TestCase):
    def test_every_relative_link_resolves(self):
        self.assertEqual(broken("."), [])



class Flow(unittest.TestCase):
    def test_readme_links_flow_and_flow_lints(self):
        self.assertIn("](docs/flow.md)", read(os.path.join(ROOT, "README.md")))
        from lib import run
        code, out = run("doc-lint", os.path.join("docs", "flow.md"))
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
