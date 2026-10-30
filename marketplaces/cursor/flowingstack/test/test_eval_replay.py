import os
import stat
import tempfile
import unittest

from lib import run, write

CASES = {
    "a.case": "id: a\narea: auth\nbug_class: logic\ntime: 2026-10\ncontract: c1\nexpected: bug\n",
    "b.case": "id: b\narea: ui\nbug_class: race\ntime: 2026-10\ncontract: c2\nexpected: refactor\n",
}
PENDING = "id: p\narea: ui\nbug_class: logic\ntime: 2026-10\ncontract: c3\nexpected: empty\n"
SKILL = "#!/bin/sh\ncase \"$1\" in c1) echo bug;; c2) echo refactor;; c3) echo {c3};; esac\n"


class Replay(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.d = os.path.join(self.t.name, "v1")
        os.makedirs(os.path.join(self.d, "pending"))
        for n, s in CASES.items():
            write(os.path.join(self.d, n), s)
        write(os.path.join(self.d, "pending", "p.case"), PENDING)
        self.assertEqual(run("eval-freeze", "lock", self.d)[0], 0)

    def tearDown(self):
        self.t.cleanup()

    def skill(self, c3):
        p = os.path.join(self.t.name, "skill")
        write(p, SKILL.format(c3=c3))
        os.chmod(p, os.stat(p).st_mode | stat.S_IXUSR)
        return p

    def replay(self, c3="empty"):
        return run("eval-replay", self.d, "--skill", self.skill(c3))

    def test_pass_exits_0(self):
        code, out = self.replay()
        self.assertEqual(code, 0, out)
        self.assertIn("cases 3 regressions 0", out)

    def test_regression_exits_1(self):
        code, out = self.replay("bug")
        self.assertEqual(code, 1, out)
        self.assertIn("regression: pending/p.case expected empty got bug", out)

    def test_tampered_freeze_exits_1(self):
        write(os.path.join(self.d, "a.case"), CASES["a.case"].replace("c1", "c2"))
        code, out = self.replay()
        self.assertEqual(code, 1, out)
        self.assertIn("mismatch: a.case", out)

    def test_missing_skill_exits_2(self):
        self.assertEqual(run("eval-replay", self.d, "--skill", os.path.join(self.t.name, "none"))[0], 2)


if __name__ == "__main__":
    unittest.main()
