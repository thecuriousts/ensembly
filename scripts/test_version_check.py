import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "version-check")
REPO = os.path.dirname(HERE)


def sh(repo, *a):
    subprocess.run(["git", "-C", repo, *a], check=True, capture_output=True,
                   env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.com",
                        "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.com"})


class VersionCheck(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.r = self.t.name
        for rel in ("marketplaces/cursor/flowingstack/.cursor-plugin/plugin.json",
                    "marketplaces/cursor/flowingstack/.claude-plugin/plugin.json",
                    "marketplaces/grokbot/ensembly/plugin.json"):
            self.put(rel, json.dumps({"name": "x", "version": "1.2.3"}))
        self.put("crates/ensembly-kernel/Cargo.toml", '[package]\nname = "k"\nversion = "0.5.1"\n')
        sh(self.r, "init", "-q")
        self.commit()

    def tearDown(self):
        self.t.cleanup()

    def put(self, rel, body):
        p = os.path.join(self.r, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w") as f:
            f.write(body)

    def commit(self):
        sh(self.r, "add", "-A")
        sh(self.r, "commit", "-qm", "c", "--allow-empty")

    def run_check(self, *a, repo=None):
        p = subprocess.run([sys.executable, SCRIPT, "--repo", repo or self.r, *a], capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr

    def test_consistent_tree_passes(self):
        code, out = self.run_check()
        self.assertEqual(code, 0, out)
        self.assertIn("flowingstack 1.2.3 flowingstack--v1.2.3 unreleased", out)
        self.assertIn("ensembly-cli 0.5.1 ensembly-cli--v0.5.1 unreleased", out)

    def test_matching_tag_passes(self):
        for tag in ("flowingstack--v1.2.3", "ensembly--v1.2.3", "ensembly-cli--v0.5.1"):
            code, out = self.run_check("--tag", tag)
            self.assertEqual(code, 0, f"{tag}: {out}")

    def test_tag_differs_from_version_fails(self):
        code, out = self.run_check("--tag", "flowingstack--v1.2.4")
        self.assertEqual(code, 1, out)
        self.assertIn("tag 1.2.4 differs from flowingstack version 1.2.3", out)

    def test_legacy_or_unknown_tag_fails(self):
        for tag in ("v0.3.0", "nope--v1.0.0"):
            self.assertEqual(self.run_check("--tag", tag)[0], 1, tag)

    def test_manifests_disagree_fails(self):
        self.put("marketplaces/cursor/flowingstack/.claude-plugin/plugin.json", json.dumps({"version": "1.2.4"}))
        code, out = self.run_check()
        self.assertEqual(code, 1, out)
        self.assertIn("flowingstack: manifests disagree", out)

    def test_change_after_release_without_bump_fails(self):
        sh(self.r, "tag", "flowingstack--v1.2.3")
        self.assertEqual(self.run_check()[0], 0)
        self.put("marketplaces/cursor/flowingstack/skills/new.md", "x\n")
        self.commit()
        code, out = self.run_check()
        self.assertEqual(code, 1, out)
        self.assertIn("changed since flowingstack--v1.2.3; bump version", out)
        for rel in ("marketplaces/cursor/flowingstack/.cursor-plugin/plugin.json",
                    "marketplaces/cursor/flowingstack/.claude-plugin/plugin.json"):
            self.put(rel, json.dumps({"version": "1.3.0"}))
        self.commit()
        self.assertEqual(self.run_check()[0], 0)

    def test_version_behind_latest_tag_fails(self):
        sh(self.r, "tag", "ensembly-cli--v0.6.0")
        code, out = self.run_check()
        self.assertEqual(code, 1, out)
        self.assertIn("version 0.5.1 is behind tag ensembly-cli--v0.6.0", out)

    def test_unreadable_manifest_exits_two(self):
        self.put("marketplaces/grokbot/ensembly/plugin.json", "{")
        self.assertEqual(self.run_check()[0], 2)
        self.put("marketplaces/grokbot/ensembly/plugin.json", json.dumps({"version": "1.2"}))
        self.assertEqual(self.run_check()[0], 2)

    def test_real_repo_is_consistent(self):
        code, out = self.run_check(repo=REPO)
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
