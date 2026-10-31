"""Replay every verified row of formal/machine.tsv against the real scripts (statechart drift check)."""
import os
import tempfile
import unittest

from lib import ROOT, read, run, write
from test_ship import FULL

GRILL = os.path.join("test", "fixtures", "grill")
BUG = os.path.join("test", "fixtures", "contract", "bug.yml")


def labels():
    out = set()
    for l in read(os.path.join(ROOT, "formal", "machine.tsv")).splitlines():
        if l and not l.startswith("#"):
            c = l.split("\t")
            if c[0] == "verified":
                out.add(c[4])
    return out


class Replay(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.ev = os.path.join(self.t.name, "evals")
        self.p = os.path.join(self.t.name, "proj")
        self.led = os.path.join(self.t.name, "ledger.tsv")
        os.makedirs(self.ev)
        for f, s in FULL:
            os.makedirs(os.path.dirname(os.path.join(self.p, f)), exist_ok=True)
            write(os.path.join(self.p, f), s)

    def lock(self):
        write(os.path.join(self.ev, "a.case"), "id: a\narea: x\nbug_class: y\ntime: 2026-10\ncontract: c\nexpected: bug\n")
        self.ok(0, "eval-freeze", "lock", self.ev)

    def tearDown(self):
        self.t.cleanup()

    def ok(self, want, *a):
        code, out = run(*a)
        self.assertEqual(code, want, out)
        return out

    def opened(self):
        return self.ok(0, "ledger", "--ledger", self.led, "open", "--contract", "c-bug", "--route", "light", "--loads", "a",
                       "--pstack", "installed").strip()

    def ship(self):
        return run("ship", "--contract", BUG, "--evals", self.ev, "--project", self.p,
                   "--plugins-dir", os.path.join(self.t.name, "plugins"), "--disk-percent", "33")

    def replay_contract_ok(self):
        self.ok(0, "contract-check", BUG)

    def replay_eval_lock(self):
        self.lock()
        self.ok(0, "eval-freeze", "verify", self.ev)

    def replay_ledger_open(self):
        self.assertTrue(self.opened())

    def replay_grill_changes(self):
        self.assertIn("verdict=changes", self.ok(1, "grill", "--findings", os.path.join(GRILL, "findings.tsv"),
                                                  "--author", "coder", "--reviewer", "grill"))

    def replay_grill_approve(self):
        self.assertIn("verdict=approve", self.ok(0, "grill", "--findings", os.path.join(GRILL, "none.tsv"),
                                                 "--author", "coder", "--reviewer", "grill"))

    def replay_close_escalate(self):
        rid = self.opened()
        self.ok(2, "ledger", "--ledger", self.led, "close", rid, "--rounds", "4")
        self.ok(0, "ledger", "--ledger", self.led, "close", rid, "--rounds", "4", "--escalate", "round cap")

    def replay_ship_ready(self):
        self.lock()
        code, out = self.ship()
        self.assertEqual(code, 0, out)
        self.assertIn("ship ready", out)

    def replay_ship_blocked(self):
        self.lock()
        write(os.path.join(self.ev, "a.case"), "tampered\n")
        code, out = self.ship()
        self.assertEqual(code, 1, out)
        self.assertIn("ship blocked: evals", out)

    def replay_ledger_close(self):
        self.ok(0, "ledger", "--ledger", self.led, "close", self.opened(), "--rounds", "3")

    def test_every_verified_row_has_a_replay(self):
        missing = sorted(l for l in labels() if not hasattr(self, "replay_" + l.replace("-", "_")))
        self.assertEqual(missing, [])

    def test_every_replay_holds_on_the_real_scripts(self):
        for l in sorted(labels()):
            with self.subTest(l):
                getattr(self, "replay_" + l.replace("-", "_"))()


if __name__ == "__main__":
    unittest.main()
