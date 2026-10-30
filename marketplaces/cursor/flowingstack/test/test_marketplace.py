import json
import os
import tempfile
import unittest

from lib import ROOT, read, run, write

REPO = os.path.dirname(os.path.dirname(os.path.dirname(ROOT)))


class Marketplace(unittest.TestCase):
    def test_root_entry(self):
        m = json.loads(read(os.path.join(REPO, ".cursor-plugin", "marketplace.json")))
        self.assertTrue(m["name"])
        self.assertTrue(m["owner"]["name"])
        self.assertNotIn("pluginRoot", m.get("metadata", {}))
        e = [p for p in m["plugins"] if p["name"] == "flowingstack"]
        self.assertEqual(len(e), 1)
        self.assertEqual(os.path.join(REPO, e[0]["source"]), ROOT)

    def test_plugin_manifest_name_matches_root_entry(self):
        m = json.loads(read(os.path.join(REPO, ".cursor-plugin", "marketplace.json")))
        e = [x for x in m["plugins"] if os.path.join(REPO, x["source"]) == ROOT]
        p = json.loads(read(os.path.join(ROOT, ".cursor-plugin", "plugin.json")))
        self.assertEqual(p["name"], e[0]["name"])
        self.assertEqual(p["name"], "flowingstack")
        self.assertTrue(os.path.isdir(os.path.join(ROOT, p["skills"])))

    def test_plugin_manifest_description(self):
        p = json.loads(read(os.path.join(ROOT, ".cursor-plugin", "plugin.json")))
        d = p.get("description", "")
        self.assertTrue(10 <= len(d) <= 200, d)
        for w in ("thecuriousts", "p10ns11y", "author"):
            self.assertNotIn(w, d.lower())
        self.assertNotIn("author", p)
        self.assertEqual(run("doc-lint", os.path.join(".cursor-plugin", "plugin.json"))[0], 0)


class Scope(unittest.TestCase):
    def test_doc_lint_never_reads_outside_root(self):
        with tempfile.TemporaryDirectory() as t:
            plug = os.path.join(t, "marketplaces", "cursor", "flowingstack")
            os.makedirs(os.path.join(t, "crates", "core"))
            os.makedirs(os.path.join(t, ".cursor-plugin"))
            os.makedirs(plug)
            write(os.path.join(plug, "banned-phrases.txt"), "leverage\n")
            write(os.path.join(plug, "a.py"), "a = 1\n")
            write(os.path.join(t, "crates", "core", "lib.rs"), "// c\n// c\nfn a() {}\n")
            write(os.path.join(t, "outside.md"), "We leverage a demo\n")
            write(os.path.join(t, ".cursor-plugin", "marketplace.json"), '{"name": "x"}\n')
            code, out = run("doc-lint", "--root", plug, "--repo-root", t, "--all")
            self.assertEqual(code, 0, out)
            self.assertIn("files 3 ", out)
            write(os.path.join(t, ".cursor-plugin", "marketplace.json"), '{"description": "x."}\n')
            self.assertEqual(run("doc-lint", "--root", plug, "--repo-root", t, "--all")[0], 1)


if __name__ == "__main__":
    unittest.main()
