"""scripts/grokbot-link: the Open in Grok Bot link must match the prompt in the doc."""
import importlib.machinery
import importlib.util
import os
import shutil
import tempfile
import unittest
from urllib.parse import unquote

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
_loader = importlib.machinery.SourceFileLoader("grokbot_link", os.path.join(HERE, "grokbot-link"))
_spec = importlib.util.spec_from_loader("grokbot_link", _loader)
gl = importlib.util.module_from_spec(_spec)
_loader.exec_module(gl)


class Link(unittest.TestCase):
    def test_repo_is_in_sync(self):
        self.assertEqual(gl.check(REPO), [])

    def test_link_decodes_to_the_prompt_with_spaces_for_newlines(self):
        text = gl.prompt(REPO)
        url = gl.link(text)
        self.assertTrue(url.startswith("https://cursor.com/grok-bot/link/v1/prompt?text="))
        self.assertNotIn("id=", url)
        self.assertEqual(unquote(url.split("?text=", 1)[1]), text.replace("\n", " "))
        self.assertNotIn("(", url)
        self.assertNotIn(")", url)

    def test_too_long_and_empty_are_refused(self):
        with self.assertRaises(ValueError):
            gl.link("a" * 1001)
        with self.assertRaises(ValueError):
            gl.link(" \n ")

    def test_drift_is_caught(self):
        with tempfile.TemporaryDirectory() as t:
            os.makedirs(os.path.join(t, "docs"))
            for rel in gl.CARRIERS:
                shutil.copy(os.path.join(REPO, rel), os.path.join(t, rel))
            doc = os.path.join(t, gl.DOC)
            with open(doc, encoding="utf-8") as f:
                body = f.read()
            with open(doc, "w", encoding="utf-8") as f:
                f.write(body.replace("Set up ensembly for me", "Set up ensembly for us", 1))
            errs = gl.check(t)
            self.assertEqual(len(errs), 2, errs)


if __name__ == "__main__":
    unittest.main()
