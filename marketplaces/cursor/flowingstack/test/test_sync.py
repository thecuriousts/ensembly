import os
import subprocess
import tempfile
import unittest

from lib import read, run


def git(cwd, *a):
    return subprocess.run(["git", "-C", cwd, *a], check=True, capture_output=True, text=True).stdout.strip()


class Sync(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.up = os.path.join(self.t.name, "up")
        self.root = os.path.join(self.t.name, "root")
        os.makedirs(os.path.join(self.up, "tstack", "skills", "harvest"))
        os.makedirs(self.root)
        with open(os.path.join(self.up, "tstack", "skills", "harvest", "SKILL.md"), "w") as f:
            f.write("harvest\n")
        git(self.up, "init", "-q")
        git(self.up, "add", ".")
        git(self.up, "-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-qm", "a")
        self.sha = git(self.up, "rev-parse", "HEAD")
        self.pins = os.path.join(self.root, "pins.tsv")
        self.pin(self.sha, "skills/harvest")

    def tearDown(self):
        self.t.cleanup()

    def pin(self, sha, dest, path="tstack/skills/harvest"):
        with open(self.pins, "w") as f:
            f.write(f"p10ns11y/plugins\t{sha}\t{path}\t{dest}\n")

    def sync(self, *a):
        return run("sync-from-upstream", "--root", self.root, "--pins", "pins.tsv", "--src", self.up, *a)

    def test_copy_then_check_clean(self):
        self.assertEqual(self.sync()[0], 0)
        self.assertEqual(read(os.path.join(self.root, "skills", "harvest", "SKILL.md")), "harvest\n")
        self.assertIn(self.sha[:7], read(os.path.join(self.root, "NOTICE.md")))
        self.assertEqual(self.sync("--check")[0], 0)

    def test_single_file_pin(self):
        self.pin(self.sha, "skills/h/references/H.md", "tstack/skills/harvest/SKILL.md")
        code, out = self.sync()
        self.assertEqual(code, 0, out)
        self.assertEqual(read(os.path.join(self.root, "skills", "h", "references", "H.md")), "harvest\n")
        self.assertEqual(self.sync("--check")[0], 0)

    def test_drift_fails_check(self):
        self.sync()
        with open(os.path.join(self.root, "skills", "harvest", "SKILL.md"), "a") as f:
            f.write("edit\n")
        code, out = self.sync("--check")
        self.assertEqual(code, 1, out)
        self.assertIn("drift", out)

    def test_extra_file_is_drift(self):
        self.sync()
        open(os.path.join(self.root, "skills", "harvest", "x.md"), "w").close()
        self.assertEqual(self.sync("--check")[0], 1)

    def test_binary_refused(self):
        with open(os.path.join(self.up, "tstack", "skills", "harvest", "b"), "wb") as f:
            f.write(b"\x7fELF\x02")
        git(self.up, "add", ".")
        git(self.up, "-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-qm", "b")
        self.pin(git(self.up, "rev-parse", "HEAD"), "skills/harvest")
        code, out = self.sync()
        self.assertEqual(code, 1, out)
        self.assertIn("binary", out)

    def test_symlink_refused(self):
        os.symlink("SKILL.md", os.path.join(self.up, "tstack", "skills", "harvest", "l"))
        git(self.up, "add", ".")
        git(self.up, "-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-qm", "c")
        self.pin(git(self.up, "rev-parse", "HEAD"), "skills/harvest")
        code, out = self.sync()
        self.assertEqual(code, 1, out)
        self.assertIn("symlink", out)

    def test_bad_pins_refused(self):
        for sha, dest in [("main", "skills/harvest"), (self.sha, "../x"), (self.sha, "/abs")]:
            with self.subTest(dest=dest, sha=sha):
                self.pin(sha, dest)
                self.assertEqual(self.sync()[0], 2)


class Pins(unittest.TestCase):
    def test_real_pins_are_full_shas(self):
        from lib import ROOT
        rows = [r.split("\t") for r in read(os.path.join(ROOT, "pins.tsv")).splitlines()]
        self.assertTrue(rows)
        for r in rows:
            self.assertEqual(len(r), 4)
            self.assertRegex(r[1], r"^[0-9a-f]{40}$")
        names = {r[2].split("/")[0] for r in rows}
        self.assertEqual(names, {"tstack", "intelli-route", "pstack-map", "aihero-map"})


if __name__ == "__main__":
    unittest.main()
