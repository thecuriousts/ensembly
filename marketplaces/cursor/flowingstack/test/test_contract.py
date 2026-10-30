import contextlib
import io
import os
import sys
import tempfile
import unittest

from lib import ROOT, read, run

FIX = os.path.join("test", "fixtures", "contract")


def check(name):
    return run("contract-check", os.path.join(FIX, name))


class Contract(unittest.TestCase):
    def test_routes_from_check_kind(self):
        for name, route in (("bug.yml", "bug"), ("feature.yml", "feature"), ("refactor.yml", "refactor"),
                            ("perf.yml", "perf"), ("eval.yml", "eval")):
            with self.subTest(name):
                code, out = check(name)
                self.assertEqual(code, 0, out)
                self.assertIn(f"route={route}", out)

    def test_missing_check_routes_empty_and_fails(self):
        code, out = check("nocheck.yml")
        self.assertEqual(code, 1)
        self.assertIn("route=empty", out)
        self.assertNotIn("route=light", out)

    def test_metric_without_number_routes_empty(self):
        code, out = check("perf-nonumber.yml")
        self.assertEqual(code, 1)
        self.assertIn("route=empty", out)

    def test_missing_id_refused(self):
        self.assertEqual(check("noid.yml")[0], 2)

    def test_paths_outside_root_refused(self):
        with tempfile.TemporaryDirectory() as t:
            f = os.path.join(t, "bug.yml")
            with open(f, "w") as w:
                w.write(read(os.path.join(ROOT, FIX, "bug.yml")))
            for p in (f, os.path.join("test", "..", "..", "flowingstack", FIX, "bug.yml")):
                with self.subTest(p):
                    code, out = run("contract-check", p)
                    self.assertEqual(code, 2, out)
                    self.assertNotIn("route=", out)

    def test_template_has_required_keys(self):
        t = read(os.path.join(ROOT, "templates", "contract.yml"))
        for k in ("id:", "input:", "output:", "check:", "reversible:", "blast:"):
            self.assertIn(k, t)


class OneOwner(unittest.TestCase):
    def test_contract_module_runs_the_check(self):
        sys.path.insert(0, os.path.join(ROOT, "scripts"))
        import contract
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = contract.main([os.path.join(FIX, "bug.yml")])
        self.assertEqual((code, out.getvalue()), (0, "id=c-bug route=bug\n"))

    def test_contract_check_is_a_thin_caller(self):
        s = read(os.path.join(ROOT, "scripts", "contract-check"))
        self.assertNotIn("parse", s)
        self.assertNotIn("route", s)


class LedgerCitesContract(unittest.TestCase):
    def test_open_from_contract_file(self):
        with tempfile.TemporaryDirectory() as t:
            f = os.path.join(t, "l.tsv")
            code, out = run("ledger", "--ledger", f, "open", "--contract-file", os.path.join(FIX, "bug.yml"), "--loads", "a", "--pstack", "installed")
            self.assertEqual(code, 0, out)
            self.assertIn("contract=c-bug\troute=bug", read(f))

    def test_open_from_contract_without_check_routes_empty(self):
        with tempfile.TemporaryDirectory() as t:
            f = os.path.join(t, "l.tsv")
            code, out = run("ledger", "--ledger", f, "open", "--contract-file", os.path.join(FIX, "nocheck.yml"), "--loads", "a", "--pstack", "installed")
            self.assertEqual(code, 0, out)
            self.assertIn("route=empty", read(f))


class Skill(unittest.TestCase):
    def test_skill_layout(self):
        s = read(os.path.join(ROOT, "skills", "task-contract", "SKILL.md"))
        for h in ("name: task-contract", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## Checks", "## Notes"):
            self.assertIn(h, s)


if __name__ == "__main__":
    unittest.main()
