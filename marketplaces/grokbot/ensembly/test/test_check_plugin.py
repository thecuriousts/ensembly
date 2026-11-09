#!/usr/bin/env python3
import json
import os
import tempfile
import unittest
from pathlib import Path

import check_plugin as c

USERS = "/" + "Users/someone/notes"
HOME = "/" + "home/someone/notes"


class Frontmatter(unittest.TestCase):
    def test_folded_and_literal_descriptions(self):
        folded = "---\nname: a\ndescription: >-\n  Use when the record\n  layer is needed.\nother: x\n---\nbody\n"
        literal = "---\nname: a\ndescription: |\n  line one\n  line two\n---\n"
        plain = "---\nname: a\ndescription: \"Plain one line.\"\n---\n"
        self.assertEqual(c.frontmatter(folded)["description"], "Use when the record layer is needed.")
        self.assertEqual(c.frontmatter(folded)["other"], "x")
        self.assertEqual(c.frontmatter(literal)["description"], "line one\nline two")
        self.assertEqual(c.frontmatter(plain)["description"], "Plain one line.")

    def test_empty_folded_description_fails(self):
        with tempfile.TemporaryDirectory() as t:
            skill = Path(t) / "a"
            skill.mkdir()
            (skill / "SKILL.md").write_text("---\nname: a\ndescription: >\n---\n```mermaid\nfill:#1e3a5f color:#ffffff\n```\n")
            with self.assertRaises(SystemExit):
                c.check_skill(skill)


class PublicScan(unittest.TestCase):
    def scan(self, text):
        with tempfile.TemporaryDirectory() as t:
            root = Path(t)
            path = root / "a.md"
            path.write_text(text)
            old = c.ROOT
            c.ROOT = root
            try:
                c.scan_public(path)
            finally:
                c.ROOT = old

    def test_users_and_home_paths_fail(self):
        for text in (USERS, HOME):
            with self.assertRaises(SystemExit):
                self.scan(text)

    def test_box_path_and_clean_text_pass(self):
        self.scan("/" + "home/box/work and laptop-1")


class CatalogScan(unittest.TestCase):
    def catalog(self, rows):
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        path = Path(t.name) / "marketplace.json"
        path.write_text(json.dumps({"name": "thecuriousts-plugins", "plugins": rows}))
        return path

    def row(self, description="Record layer on laptop-1"):
        return {"name": "ensembly", "source": c.ROW_SOURCE, "description": description}

    def test_repo_catalog_clean(self):
        c.check_catalog()

    def test_leak_fixture_fails(self):
        with self.assertRaises(SystemExit):
            c.check_catalog(Path(__file__).parent / "fixtures" / "catalog-leak.json")

    def test_alias_row_passes(self):
        c.check_catalog(self.catalog([self.row("Record layer on laptop-1, laptop-2, mac-mini, phone")]))

    def test_each_leak_kind_fails(self):
        leaks = (HOME, USERS, "100." + "101.2.3", "a.b@" + "gmail.com", "box." + "local",
                 "0f1e2d3c-4b5a-" + "6978-8796-a5b4c3d2e1f0")
        for leak in leaks:
            with self.subTest(leak):
                with self.assertRaises(SystemExit):
                    c.check_catalog(self.catalog([self.row("see " + leak)]))

    def test_leak_in_other_row_fails(self):
        other = {"name": "x", "source": "marketplaces/cursor/x", "description": "see " + HOME}
        with self.assertRaises(SystemExit):
            c.check_catalog(self.catalog([self.row(), other]))

    def test_missing_row_fails(self):
        with self.assertRaises(SystemExit):
            c.check_catalog(self.catalog([]))

    def test_deny_file_host_fails(self):
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("zeta-host\n")
        self.addCleanup(os.unlink, f.name)
        path = self.catalog([self.row("built on zeta-host")])
        old = os.environ.get("PRIVACY_DENY_FILE")
        os.environ["PRIVACY_DENY_FILE"] = f.name
        try:
            with self.assertRaises(SystemExit):
                c.check_catalog(path)
        finally:
            if old is None:
                os.environ.pop("PRIVACY_DENY_FILE")
            else:
                os.environ["PRIVACY_DENY_FILE"] = old


if __name__ == "__main__":
    unittest.main()
