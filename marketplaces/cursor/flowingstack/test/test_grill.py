import hashlib
import os
import tempfile
import unittest

from lib import ROOT, read, run

FIX = os.path.join("test", "fixtures", "grill")


def grill(name, author="coder", reviewer="grill", *extra):
    return run("grill", "--findings", os.path.join(FIX, name), "--author", author, "--reviewer", reviewer, *extra)


class Score(unittest.TestCase):
    def test_reproducible_across_two_runs(self):
        a, b = grill("findings.tsv"), grill("findings.tsv")
        self.assertEqual(a, b)
        self.assertIn("score=", a[1])

    def test_known_score(self):
        code, out = grill("findings.tsv")
        self.assertIn("score=65", out)
        self.assertIn("verdict=changes", out)
        self.assertEqual(code, 1)

    def test_clean_approves(self):
        code, out = grill("none.tsv")
        self.assertEqual(code, 0, out)
        self.assertIn("score=100 verdict=approve", out)

    def test_blocker_never_approves(self):
        code, out = grill("blocker.tsv")
        self.assertEqual(code, 1)
        self.assertIn("verdict=changes", out)

    def test_self_review_cannot_approve(self):
        code, out = grill("none.tsv", "coder", "coder")
        self.assertEqual(code, 3)
        self.assertIn("verdict=screen", out)
        self.assertNotIn("approve", out)

    def test_unknown_rubric_id_refused(self):
        self.assertEqual(grill("unknown.tsv")[0], 2)

    def test_rubric_fixed_and_hashed(self):
        h = hashlib.sha256(open(os.path.join(ROOT, "references", "rubric.md"), "rb").read()).hexdigest()[:12]
        self.assertIn(f"rubric={h}", grill("none.tsv")[1])
        self.assertNotEqual(grill("none.tsv", "coder", "grill", "--rubric", "x")[0], 0)


class Peers(unittest.TestCase):
    def test_names_installed_peer_skills(self):
        with tempfile.TemporaryDirectory() as t:
            os.makedirs(os.path.join(t, "interrogate"))
            open(os.path.join(t, "interrogate", "SKILL.md"), "w").close()
            out = grill("none.tsv", "coder", "grill", "--skills-dir", t)[1]
            self.assertIn("uses: interrogate", out)
            self.assertIn("missing: blast-radius", out)


class Skill(unittest.TestCase):
    def test_skill_layout(self):
        s = read(os.path.join(ROOT, "skills", "grill-rubric", "SKILL.md"))
        for h in ("name: grill-rubric", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## Checks", "## Notes"):
            self.assertIn(h, s)


if __name__ == "__main__":
    unittest.main()
