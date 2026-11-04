import os
import unittest

from lib import ROOT, read

REF = "references/verify.md"
RULES = ("done before work", "confirmed", "suspected", "not checked", "Verified", "Not met", "Not checked", "arithmetic or test", "read only")
USERS = ("grill-rubric", "task-contract")


class Verify(unittest.TestCase):
    def test_reference_keeps_every_rule(self):
        s = read(os.path.join(ROOT, REF))
        for r in RULES:
            with self.subTest(r):
                self.assertIn(r, s)

    def test_review_skills_cite_reference(self):
        for d in USERS:
            with self.subTest(d):
                self.assertIn(REF, read(os.path.join(ROOT, "skills", d, "SKILL.md")))


if __name__ == "__main__":
    unittest.main()
