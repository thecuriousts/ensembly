import os
import tempfile
import unittest

from lib import ROOT, read, run

FIX = os.path.join(ROOT, "test", "fixtures", "ledger", "misroute.tsv")


class Ledger(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.f = os.path.join(self.t.name, "ledger.tsv")

    def tearDown(self):
        self.t.cleanup()

    def led(self, *a):
        return run("ledger", "--ledger", self.f, *a)

    def open_run(self, *extra):
        return self.led("open", "--contract", "c1", "--route", "light", "--loads", "a,b", "--pstack", "installed", *extra)

    def test_open_writes_row_before_run(self):
        code, out = self.open_run()
        self.assertEqual(code, 0, out)
        rid = out.strip()
        rows = read(self.f).splitlines()
        self.assertEqual(rows[0].split("\t")[:2], ["open", rid])
        self.assertIn("contract=c1", rows[0])

    def test_missing_contract_refused(self):
        code, _ = self.led("open", "--route", "light", "--loads", "a", "--pstack", "installed")
        self.assertEqual(code, 2)
        self.assertFalse(os.path.exists(self.f))

    def test_more_than_four_loads_refused(self):
        self.assertEqual(self.open_run("--loads", "a,b,c,d,e")[0], 2)

    def test_close_without_open_refused(self):
        self.assertEqual(self.led("close", "rX", "--opened", "a")[0], 2)

    def test_tab_in_value_refused(self):
        self.assertEqual(self.led("open", "--contract", "c\t1", "--route", "light", "--loads", "a", "--pstack", "installed")[0], 2)

    def test_round_trip(self):
        rid = self.open_run()[1].strip()
        self.assertEqual(self.led("close", rid, "--opened", "a,b,c")[0], 0)
        code, out = self.led("query", "missed-load")
        self.assertEqual(out.split(), [rid])

    def test_claim_status_values(self):
        rid = self.open_run()[1].strip()
        for s in ("verified", "unverified", "doubted"):
            self.assertEqual(self.led("claim", rid, "--text", "t", "--status", s, "--source", "ci")[0], 0)
        self.assertEqual(self.led("claim", rid, "--text", "t", "--status", "sure", "--source", "ci")[0], 2)
        self.assertEqual(self.led("claim", rid, "--text", "t", "--status", "doubted")[0], 2)

    def test_compact_keeps_claims_and_last_state(self):
        rid = self.open_run()[1].strip()
        self.led("claim", rid, "--text", "fast", "--status", "doubted", "--source", "bench")
        self.led("close", rid, "--opened", "a")
        self.led("claim", rid, "--text", "fast", "--status", "verified", "--source", "bench2")
        self.led("claim", rid, "--text", "safe", "--status", "doubted", "--source", "review")
        self.assertEqual(self.led("compact")[0], 0)
        rows = [r.split("\t") for r in read(self.f).splitlines()]
        kinds = [r[0] for r in rows]
        self.assertEqual(kinds.count("open"), 1)
        self.assertEqual(kinds.count("close"), 1)
        claims = {r[3]: r[4] for r in rows if r[0] == "claim"}
        self.assertEqual(claims, {"text=fast": "status=verified", "text=safe": "status=doubted"})


class Misroute(unittest.TestCase):
    def q(self, name):
        code, out = run("ledger", "--ledger", FIX, "query", name)
        self.assertEqual(code, 0, out)
        return out.split()

    def test_queries(self):
        self.assertEqual(self.q("missed-load"), ["r1"])
        self.assertEqual(self.q("unused-load"), ["r2"])
        self.assertEqual(self.q("playbook-switch"), ["r3"])
        self.assertEqual(self.q("premise"), ["r3"])
        self.assertEqual(self.q("override"), ["r3"])
        self.assertEqual(self.q("unrun-check"), ["r3"])

    def test_unknown_query_refused(self):
        self.assertEqual(run("ledger", "--ledger", FIX, "query", "vibes")[0], 2)


class Skill(unittest.TestCase):
    def test_skill_layout(self):
        s = read(os.path.join(ROOT, "skills", "run-ledger", "SKILL.md"))
        for h in ("name: run-ledger", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## States", "## Checks", "## Notes"):
            self.assertIn(h, s)


if __name__ == "__main__":
    unittest.main()
