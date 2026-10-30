import os
import unittest

from lib import run


def lint(name):
    return run("brief-lint", os.path.join("test", "fixtures", "brief", name))


class Brief(unittest.TestCase):
    def test_complete_brief_passes(self):
        code, out = lint("ok.md")
        self.assertEqual((code, out.strip()), (0, "brief ok"))

    def test_missing_skills_refused(self):
        self.assertEqual(lint("noskills.md"), (1, "brief missing: skills\n"))

    def test_missing_snapshot_refused(self):
        self.assertEqual(lint("nosnapshot.md"), (1, "brief missing: snapshot\n"))

    def test_missing_pr_body_rule_refused(self):
        self.assertEqual(lint("nopr.md"), (1, "brief missing: pr-body\n"))

    def test_pr_body_over_120_words_refused(self):
        self.assertEqual(lint("longpr.md"), (1, "brief missing: pr-body\n"))


if __name__ == "__main__":
    unittest.main()
