import json
import os
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from metrics import compute
from render import render

FIX = os.path.join(HERE, "fixtures")


def load(name):
    with open(os.path.join(FIX, name)) as handle:
        return json.load(handle)


class RenderDoc(unittest.TestCase):
    def setUp(self):
        self.md = render(compute(load("mixed.json"), "2026-10-01", "2026-10-31"))

    def test_leads_with_themed_mermaid(self):
        head = self.md.split("## Definitions", 1)[0]
        self.assertTrue(self.md.startswith("# Public engineering scoreboard\n"))
        self.assertIn("```mermaid\n", head)
        self.assertIn('"theme": "base"', head)
        self.assertIn("flowchart TB", head)
        self.assertIn("primaryColor", head)
        self.assertLess(self.md.index("```mermaid"), self.md.index("## Definitions"))
        self.assertLess(self.md.index("## Definitions"), self.md.index("## Results"))

    def test_table_and_prose(self):
        self.assertIn("| Repository | PRs merged | First-pass CI |", self.md)
        self.assertIn("| alice/alpha | 3 | 67% (2/3) | 1 |", self.md)
        self.assertIn("| **Total** | 4 |", self.md)
        self.assertIn("Scanned 3 public repositories; 2 had a merged pull request", self.md)
        self.assertIn("1 Oct 2026 to 31 Oct 2026 UTC", self.md)
        self.assertIn("python3 scripts/scoreboard/scoreboard --since 2026-10-01 --until 2026-10-31", self.md)

    def test_no_private_names(self):
        self.assertNotIn("hidden", self.md)
        self.assertNotIn("must not appear", self.md)

    def test_special_repo_name_uses_trailing_slash(self):
        leaf = "ensembl" + "y"
        snapshot = load("empty.json")
        snapshot["repos"] = [{"full_name": f"owner/{leaf}", "visibility": "public", "private": False}]
        snapshot["pulls"] = [{
            "repo": f"owner/{leaf}",
            "number": 1,
            "title": "Add docs",
            "body": "",
            "labels": [],
            "created_at": "2026-10-10T10:00:00Z",
            "merged_at": "2026-10-10T11:00:00Z",
            "check_runs": [{"name": "ci", "started_at": "2026-10-10T10:05:00Z", "conclusion": "success"}],
        }]
        md = render(compute(snapshot, "2026-10-01", "2026-10-31"))
        self.assertIn(f"owner/{leaf}/", md)
        self.assertIsNone(re.search(r"(?i)\bensembl" + r"y\b(?!/)", md))


class CliFixture(unittest.TestCase):
    def test_cli_is_deterministic_for_a_window(self):
        import subprocess
        cmd = [
            sys.executable,
            os.path.join(ROOT, "scoreboard"),
            "--fixture", os.path.join(FIX, "mixed.json"),
            "--since", "2026-10-01",
            "--until", "2026-10-31",
        ]
        first = subprocess.run(cmd, capture_output=True, text=True, check=True)
        second = subprocess.run(cmd, capture_output=True, text=True, check=True)
        self.assertEqual(first.stdout, second.stdout)
        self.assertIn("| **Total** | 4 |", first.stdout)
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "scoreboard.md")
            subprocess.run(cmd + ["--output", out], check=True, capture_output=True, text=True)
            with open(out) as handle:
                self.assertEqual(handle.read(), first.stdout)


if __name__ == "__main__":
    unittest.main()
