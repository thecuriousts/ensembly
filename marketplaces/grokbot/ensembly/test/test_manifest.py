import json
import os
import unittest

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parents[2]


class Manifest(unittest.TestCase):
    def test_marketplace_owner(self):
        catalog = json.loads((REPO / ".cursor-plugin/marketplace.json").read_text())
        self.assertEqual(catalog["owner"]["name"], "Peramanathan Sathyamoorthy")
        self.assertNotIn("email", catalog["owner"])
        row = [p for p in catalog["plugins"] if p["name"] == "ensembly"][0]
        self.assertEqual(row["source"], "marketplaces/grokbot/ensembly")

    def test_plugin_manifest_fields(self):
        for name in (".cursor-plugin/plugin.json", "plugin.json"):
            path = ROOT / name
            p = json.loads(path.read_text())
            self.assertEqual(p["name"], "ensembly")
            self.assertRegex(p.get("version", ""), r"^\d+\.\d+\.\d+$")
            self.assertEqual(p["author"]["name"], "Peramanathan Sathyamoorthy")
            self.assertEqual(p.get("publisher"), "Peramanathan Sathyamoorthy")
            self.assertEqual(p.get("logo"), "assets/logo.svg")
            self.assertTrue((ROOT / p["logo"]).is_file())
            self.assertNotIn("email", p["author"])


if __name__ == "__main__":
    unittest.main()