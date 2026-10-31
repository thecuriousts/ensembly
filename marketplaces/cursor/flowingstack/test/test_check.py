import os
import subprocess
import tempfile
import unittest

from lib import ROOT, run


def check(*a):
    p = subprocess.run([os.path.join(ROOT, "scripts", "check"), *a], capture_output=True, text=True, timeout=60)
    return p.returncode, p.stdout + p.stderr


class Args(unittest.TestCase):
    def test_src_equals_form_parsed(self):
        with tempfile.TemporaryDirectory() as t:
            gone = os.path.join(t, "gone")
            code, out = check(f"--src={gone}")
            self.assertEqual(code, 2, out)
            self.assertIn(f"{gone}: not a directory", out)

    def test_missing_value_is_usage(self):
        code, out = check("--src")
        self.assertEqual(code, 2, out)
        self.assertIn("usage: check --src DIR", out)

    def test_no_args_is_usage(self):
        self.assertEqual(check()[0], 2)


class DocLintMissing(unittest.TestCase):
    def test_missing_path_exits_2_without_traceback(self):
        with tempfile.TemporaryDirectory() as t:
            code, out = run("doc-lint", "nope.md", cwd=t)
            self.assertEqual(code, 2, out)
            self.assertNotIn("Traceback", out)
            self.assertIn("nope.md: not found under plugin root", out)


if __name__ == "__main__":
    unittest.main()
