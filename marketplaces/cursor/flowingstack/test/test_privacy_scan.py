import os
import tempfile
import unittest

from lib import run

RED = {
    "home": "see /home/someone/x", "mac-home": "see /Users/someone/x", "box": "under /workspace/dev/x",
    "store": "in /cursor/stores/abc", "cgnat": "ssh 100.101.2.3", "email": "Co-authored-by: A <a.b@gmail.com>",
    "phone": "call +46 70 123 45 67", "finance": "the IBAN is here", "health": "a diagnosis note",
    "cv": "attach my CV", "kanithanj": "kanithanj rows", "ensembly": "Ensembly member data",
}
OK = ["laptop-1 and mac-mini", "Co-authored-by: A <1+a@users.noreply.github.com>", "ip 10.0.0.1 and 100.1.2.3",
      "v1.2.3 at https://example.com/home", "contact a@example.com"]


class Text(unittest.TestCase):
    def test_red_lines_fail(self):
        for name, line in RED.items():
            with self.subTest(name):
                code, out = run("privacy-scan", "--stdin", stdin=line + "\n")
                self.assertEqual(code, 1, out)

    def test_clean_lines_pass(self):
        for line in OK:
            with self.subTest(line):
                code, out = run("privacy-scan", "--stdin", stdin=line + "\n")
                self.assertEqual(code, 0, out)

    def test_deny_file_host_names(self):
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("zeta-host\n")
        code, out = run("privacy-scan", "--deny", f.name, "--stdin", stdin="rebuild on zeta-host\n")
        os.unlink(f.name)
        self.assertEqual(code, 1, out)
        self.assertNotIn("zeta-host", out)


class Files(unittest.TestCase):
    def test_binary_refused(self):
        code, out = run("privacy-scan", os.path.join("test", "fixtures", "privacy", "elf.bin"))
        self.assertEqual(code, 1, out)
        self.assertIn("binary", out)

    def test_own_tree_clean(self):
        code, out = run("privacy-scan", "--all")
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
