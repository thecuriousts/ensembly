import os
import subprocess
import tempfile
import unittest

from lib import ROOT, read, run, write

CASES = {
    "a.case": "id: a\narea: auth\nbug_class: logic\ntime: 2026-09\ncontract: c1\nexpected: bug\n",
    "b.case": "id: b\narea: auth\nbug_class: race\ntime: 2026-10\ncontract: c2\nexpected: refactor\n",
    "c.case": "id: c\narea: ui\nbug_class: logic\ntime: 2026-10\ncontract: c3\nexpected: empty\n",
}


def git(cwd, *a):
    return subprocess.run(["git", "-C", cwd, "-c", "user.name=t", "-c", "user.email=t@example.com", *a],
                          check=True, capture_output=True, text=True).stdout.strip()


class Freeze(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.d = os.path.join(self.t.name, "evals", "v1")
        os.makedirs(self.d)
        for n, s in CASES.items():
            write(os.path.join(self.d, n), s)

    def tearDown(self):
        self.t.cleanup()

    def ef(self, *a):
        return run("eval-freeze", *a)

    def test_lock_then_verify(self):
        self.assertEqual(self.ef("lock", self.d)[0], 0)
        self.assertIn("a.case", read(os.path.join(self.d, "FROZEN.sha256")))
        self.assertEqual(self.ef("verify", self.d)[0], 0)

    def test_hash_mismatch_fails(self):
        self.ef("lock", self.d)
        write(os.path.join(self.d, "a.case"), CASES["a.case"].replace("bug\n", "feature\n"))
        code, out = self.ef("verify", self.d)
        self.assertEqual(code, 1)
        self.assertIn("a.case", out)

    def test_added_or_removed_case_fails(self):
        self.ef("lock", self.d)
        os.remove(os.path.join(self.d, "b.case"))
        self.assertEqual(self.ef("verify", self.d)[0], 1)

    def test_unlocked_verify_fails(self):
        self.assertEqual(self.ef("verify", self.d)[0], 1)

    def test_case_without_split_keys_refused(self):
        write(os.path.join(self.d, "d.case"), "id: d\nexpected: bug\n")
        self.assertEqual(self.ef("lock", self.d)[0], 2)

    def test_override_becomes_pending_case(self):
        self.ef("lock", self.d)
        code, out = self.ef("override", self.d, "--id", "o1", "--contract", "c9", "--expected", "bug",
                            "--area", "auth", "--bug-class", "logic", "--time", "2026-10")
        self.assertEqual(code, 0, out)
        self.assertIn("expected: bug", read(os.path.join(self.d, "pending", "o1.case")))
        self.assertEqual(self.ef("verify", self.d)[0], 0)
        self.assertEqual(self.ef("override", self.d, "--id", "a", "--contract", "c", "--expected", "bug",
                                 "--area", "x", "--bug-class", "y", "--time", "2026-10")[0], 2)

    def ov(self, **kw):
        f = {"id": "o2", "contract": "c", "expected": "bug", "area": "a", "bug-class": "b", "time": "2026-10", **kw}
        return self.ef("override", self.d, *[x for k, v in f.items() for x in ("--" + k, v)])

    def test_override_id_cannot_leave_frozen_dir(self):
        for bad in ("../../x", "../x", "a/b", os.path.join(self.t.name, "abs"), "..", ".", ""):
            with self.subTest(bad):
                self.assertEqual(self.ov(id=bad)[0], 2)
        self.assertFalse(os.path.exists(os.path.join(self.t.name, "x.case")))
        self.assertFalse(os.path.exists(os.path.join(self.t.name, "evals", "x.case")))
        self.assertFalse(os.path.exists(os.path.join(self.t.name, "abs.case")))

    def test_override_refuses_control_chars(self):
        for i, (k, c) in enumerate((k, c) for k in ("expected", "contract", "area", "bug-class", "time") for c in "\n\t\r"):
            with self.subTest(k=k, c=repr(c)):
                self.assertEqual(self.ov(**{"id": f"o{i}", k: "bug" + c + "id: evil"})[0], 2)
                self.assertFalse(os.path.exists(os.path.join(self.d, "pending", f"o{i}.case")))

    def test_split(self):
        code, out = self.ef("split", self.d, "--by", "area")
        self.assertEqual(code, 0)
        self.assertEqual(out.split("\n")[:2], ["auth\t2", "ui\t1"])
        self.assertIn("2026-10\t2", self.ef("split", self.d, "--by", "time")[1])
        self.assertIn("logic\t2", self.ef("split", self.d, "--by", "bug_class")[1])

    def test_diff_touching_frozen_fails(self):
        repo = self.t.name
        git(repo, "init", "-q")
        self.ef("lock", self.d)
        write(os.path.join(repo, "src.py"), "a = 1\n")
        git(repo, "add", ".")
        git(repo, "commit", "-qm", "base")
        base = git(repo, "rev-parse", "HEAD")
        write(os.path.join(repo, "src.py"), "a = 2\n")
        git(repo, "commit", "-qam", "code")
        self.assertEqual(self.ef("diff-check", "--repo", repo, "--base", base, "evals/v1")[0], 0)
        write(os.path.join(self.d, "c.case"), CASES["c.case"] + "note: x\n")
        code, out = self.ef("diff-check", "--repo", repo, "--base", base, "evals/v1")
        self.assertEqual(code, 1, out)
        self.assertIn("evals/v1/c.case", out)


class Skill(unittest.TestCase):
    def test_skill_layout(self):
        s = read(os.path.join(ROOT, "skills", "eval-freeze", "SKILL.md"))
        for h in ("name: eval-freeze", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## States", "## Checks", "## Notes"):
            self.assertIn(h, s)


if __name__ == "__main__":
    unittest.main()
