import json
import os
import shutil
import tempfile
import unittest

from lib import ROOT, run, write

OFF = {"FLOWINGSTACK_DOCTOR_REMOTE": "https://127.0.0.1:9/none.git", "FLOWINGSTACK_OFFLINE": None}


class Rails(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.home = os.path.join(self.t.name, "home")
        os.makedirs(self.home)
        self.env = {"HOME": self.home, "CLAUDE_CONFIG_DIR": os.path.join(self.home, ".claude"), **OFF}

    def tearDown(self):
        self.t.cleanup()

    def doc(self, *a, env=None, root=None):
        return run("doctor", "--rails", *a, env={**self.env, **(env or {}), **({"FLOWINGSTACK_ROOT": root} if root else {})})

    def version(self):
        with open(os.path.join(ROOT, ".cursor-plugin", "plugin.json")) as f:
            return json.load(f)["version"]

    def installed(self, path):
        d = os.path.join(self.home, ".claude", "plugins")
        os.makedirs(d, exist_ok=True)
        write(os.path.join(d, "installed_plugins.json"), json.dumps({"version": 2, "plugins": {
            "flowingstack@local": [{"scope": "user", "installPath": path, "version": self.version()}]}}))

    def test_clean_profile_offline_exits_zero(self):
        code, out = self.doc()
        self.assertEqual(code, 0, out)
        self.assertRegex(out, rf"(?m)^flowingstack {self.version()}$")
        self.assertRegex(out, rf"(?m)^manifest claude {self.version()}$")
        self.assertRegex(out, r"(?m)^harness claude-code absent flowingstack not-installed$")
        self.assertRegex(out, r"(?m)^harness cursor absent flowingstack not-installed$")
        self.assertRegex(out, r"(?m)^latest offline$")

    def test_offline_flag_skips_lookup(self):
        code, out = self.doc("--offline")
        self.assertEqual(code, 0, out)
        self.assertRegex(out, r"(?m)^latest skipped$")

    def test_installed_plugin_reported(self):
        self.installed(self.t.name)
        code, out = self.doc()
        self.assertEqual(code, 0, out)
        self.assertRegex(out, rf"(?m)^harness claude-code flowingstack@local {self.version()} user$")

    def test_cursor_link_reported(self):
        d = os.path.join(self.home, ".cursor", "plugins", "local")
        os.makedirs(d)
        os.symlink(ROOT, os.path.join(d, "flowingstack"))
        code, out = self.doc()
        self.assertEqual(code, 0, out)
        self.assertRegex(out, rf"(?m)^harness cursor flowingstack {self.version()}$")

    def test_missing_install_path_fails(self):
        self.installed(os.path.join(self.t.name, "gone"))
        code, out = self.doc()
        self.assertEqual(code, 1, out)
        self.assertRegex(out, r"(?m)^fail harness claude-code flowingstack@local")

    def test_unreadable_installed_plugins_fails(self):
        d = os.path.join(self.home, ".claude", "plugins")
        os.makedirs(d)
        write(os.path.join(d, "installed_plugins.json"), "{")
        code, out = self.doc()
        self.assertEqual(code, 1, out)
        self.assertRegex(out, r"(?m)^fail harness claude-code\t")

    def copy(self):
        r = os.path.join(self.t.name, "plug")
        shutil.copytree(ROOT, r, ignore=shutil.ignore_patterns("test", "docs", "skills", "formal", "references"))
        return r

    def test_manifest_drift_fails(self):
        r = self.copy()
        p = os.path.join(r, ".claude-plugin", "plugin.json")
        with open(p) as f:
            m = json.load(f)
        m["version"] = "9.9.9"
        write(p, json.dumps(m))
        code, out = self.doc(root=r)
        self.assertEqual(code, 1, out)
        self.assertRegex(out, r"(?m)^fail manifest claude 9\.9\.9\t")

    def test_unreadable_cursor_manifest_exits_two(self):
        r = self.copy()
        write(os.path.join(r, ".cursor-plugin", "plugin.json"), "{")
        code, out = self.doc(root=r)
        self.assertEqual(code, 2, out)

    def test_manifests_agree_in_tree(self):
        with open(os.path.join(ROOT, ".claude-plugin", "plugin.json")) as f:
            c = json.load(f)
        self.assertEqual((c["name"], c["version"]), ("flowingstack", self.version()))


if __name__ == "__main__":
    unittest.main()
