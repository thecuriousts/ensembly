#!/usr/bin/env python3
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


if __name__ == "__main__":
    unittest.main()
