import os
import tempfile
import unittest

from lib import ROOT, read, run, write

FULL = ((".cursor/rules/pstack-models.mdc", "m\n"), ("AGENTS.md", "hosts: laptop-1\n"), (".github/workflows/ci.yml", "on: push\n"),
        (".cursor/hooks.json", "{}\n"), (".cursor/skills/verify-app/SKILL.md", "v\n"), ("../plugins/pstack/SKILL.md", "p\n"),
        ("../plugins/craft/SKILL.md", "c\n"))


class Ship(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.p = os.path.join(self.t.name, "proj")
        self.ev = os.path.join(self.t.name, "evals")
        os.makedirs(self.ev)
        for f, s in FULL:
            os.makedirs(os.path.dirname(os.path.join(self.p, f)), exist_ok=True)
            write(os.path.join(self.p, f), s)
        write(os.path.join(self.ev, "a.case"), "id: a\narea: x\nbug_class: y\ntime: 2026-10\ncontract: c\nexpected: bug\n")
        run("eval-freeze", "lock", self.ev)

    def tearDown(self):
        self.t.cleanup()

    def ship(self, contract="bug.yml"):
        return run("ship", "--contract", os.path.join("test", "fixtures", "contract", contract), "--evals", self.ev,
                   "--project", self.p, "--plugins-dir", os.path.join(self.t.name, "plugins"), "--disk-percent", "33")

    def test_all_clean_prints_ready(self):
        code, out = self.ship()
        self.assertEqual(code, 0, out)
        self.assertEqual(out.strip().splitlines()[-1], "ship ready: contract evals privacy doc-lint doctor")

    def test_bad_contract_blocks(self):
        code, out = self.ship("nocheck.yml")
        self.assertEqual(code, 1, out)
        self.assertEqual(out.strip().splitlines()[-1], "ship blocked: contract")

    def test_unfrozen_evals_block(self):
        write(os.path.join(self.ev, "a.case"), "tampered\n")
        code, out = self.ship()
        self.assertEqual(code, 1, out)
        self.assertIn("ship blocked: evals", out)

    def test_never_merges(self):
        self.assertNotIn("merge", read(os.path.join(ROOT, "scripts", "ship")))


if __name__ == "__main__":
    unittest.main()
