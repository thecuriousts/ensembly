import hashlib
import os
import pathlib
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
        h = hashlib.sha256(pathlib.Path(ROOT, "references", "rubric.md").read_bytes()).hexdigest()[:12]
        self.assertIn(f"rubric={h}", grill("none.tsv")[1])
        self.assertNotEqual(grill("none.tsv", "coder", "grill", "--rubric", "x")[0], 0)


class Scope(unittest.TestCase):
    def test_findings_outside_root_refused(self):
        with tempfile.TemporaryDirectory() as t:
            f = os.path.join(t, "f.tsv")
            with open(f, "w") as w:
                w.write("R6\tminor\n")
            for p in (f, os.path.join("test", "..", "..", "flowingstack", FIX, "none.tsv")):
                with self.subTest(p):
                    code, out = run("grill", "--findings", p, "--author", "a", "--reviewer", "b")
                    self.assertEqual(code, 2, out)
                    self.assertNotIn("score=", out)


class Provisional(unittest.TestCase):
    def test_weights_threshold_factors_marked_provisional(self):
        r = read(os.path.join(ROOT, "references", "rubric.md"))
        self.assertIn("provisional", r)
        self.assertIn("noise floor", r)
        self.assertIn("first real-work evaluation", r)

    def test_blocker_score_documented(self):
        r = read(os.path.join(ROOT, "references", "rubric.md"))
        self.assertRegex(r, r"(?m)^\| blocker \| 0, the verdict decides \|$")


class Peers(unittest.TestCase):
    def test_names_installed_peer_skills(self):
        with tempfile.TemporaryDirectory() as t:
            os.makedirs(os.path.join(t, "interrogate"))
            open(os.path.join(t, "interrogate", "SKILL.md"), "w").close()
            out = grill("none.tsv", "coder", "grill", "--skills-dir", t)[1]
            self.assertIn("uses: interrogate", out)
            self.assertIn("missing: blast-radius", out)

    def test_relative_skills_dir_resolves_from_plugin_root(self):
        with tempfile.TemporaryDirectory() as t:
            code, out = run("grill", "--findings", os.path.join(FIX, "none.tsv"), "--author", "a", "--reviewer", "b",
                            "--skills-dir", os.path.join("test", "fixtures", "peers"), cwd=t)
            self.assertIn("uses: interrogate", out)


class Skill(unittest.TestCase):
    def test_skill_layout(self):
        s = read(os.path.join(ROOT, "skills", "grill-rubric", "SKILL.md"))
        for h in ("name: grill-rubric", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## Checks", "## Notes"):
            self.assertIn(h, s)


if __name__ == "__main__":
    unittest.main()
