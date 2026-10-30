import os
import tempfile
import unittest

from lib import FIX, ROOT, run, write

OVER = {
    "prose-first.md": "order", "ratio.md": "ratio", "prose-cap.md": "prose-cap",
    "demo.md": "banned-word", "gate.md": "banned-word", "ai.md": "banned-word", "agi.md": "banned-word",
    "filler.md": "filler", "filler2.md": "filler",
    "punct-heading.md": "punct", "punct-cell.md": "punct", "punct-list.md": "punct",
    "bang.md": "redundant", "ellipsis.md": "redundant", "dspace.md": "redundant",
    "trail.md": "redundant", "bold.md": "redundant", "html.md": "html-comment",
    "restate.py": "comment-restate", "dense.py": "comment-density", "dense.mjs": "comment-density",
    "dense.sh": "comment-density", "comment.json": "json-comment", "desc.json": "punct",
    "dense.yml": "comment-density", "mermaid-plain.md": "mermaid-theme",
    "comment-line.py": "comment-line", "comment-line.sh": "comment-line", "comment-line.js": "comment-line",
}


class Over(unittest.TestCase):
    def test_each_red_fixture_fails_with_rule(self):
        for name, rule in OVER.items():
            with self.subTest(name):
                code, out = run("doc-lint", os.path.join("test", "fixtures", "over", name))
                self.assertEqual(code, 1, out)
                self.assertIn(f": {rule}:", out)

    def test_every_red_fixture_listed(self):
        self.assertEqual(sorted(os.listdir(os.path.join(FIX, "over"))), sorted(OVER))


class MustNotFire(unittest.TestCase):
    def test_each_exemption_fixture_passes(self):
        for name in sorted(os.listdir(os.path.join(FIX, "ok"))):
            with self.subTest(name):
                code, out = run("doc-lint", os.path.join("test", "fixtures", "ok", name))
                self.assertEqual(code, 0, out)


class Passive(unittest.TestCase):
    def test_passive_warns_only(self):
        code, out = run("doc-lint", "--stdin-md", stdin="# x\n\n```agentflow-beta LR\n  a --> b\n  b --> c\n```\n\nThe row was added\n")
        self.assertEqual(code, 0, out)
        self.assertIn("warn: passive", out)


class Baseline(unittest.TestCase):
    def test_baseline_suppresses_path_rule(self):
        p = os.path.join("test", "fixtures", "over", "filler.md")
        code, out = run("doc-lint", "--baseline", os.path.join("test", "fixtures", "baseline.tsv"), p)
        self.assertEqual(code, 0, out)
        self.assertIn("baselined 1", out)

    def test_growth_past_baselined_count_fails(self):
        with tempfile.TemporaryDirectory() as t:
            doc = open(os.path.join(FIX, "over", "filler.md")).read()
            write(os.path.join(t, "a.md"), doc)
            write(os.path.join(t, "b.tsv"), "a.md\tfiller\t1\n")
            self.assertEqual(run("doc-lint", "--root", t, "--baseline", "b.tsv", "a.md")[0], 0)
            write(os.path.join(t, "a.md"), doc + "\nWe leverage the queue\n")
            code, out = run("doc-lint", "--root", t, "--baseline", "b.tsv", "a.md")
            self.assertEqual(code, 1, out)
            self.assertIn("a.md:14: filler", out)

    def test_baseline_row_without_count_refused(self):
        with tempfile.TemporaryDirectory() as t:
            write(os.path.join(t, "a.md"), open(os.path.join(FIX, "over", "filler.md")).read())
            write(os.path.join(t, "b.tsv"), "a.md\tfiller\n")
            self.assertEqual(run("doc-lint", "--root", t, "--baseline", "b.tsv", "a.md")[0], 2)

    def test_write_baseline_records_counts(self):
        with tempfile.TemporaryDirectory() as t:
            write(os.path.join(t, "a.md"), open(os.path.join(FIX, "over", "filler.md")).read() + "\nWe leverage the queue\n")
            run("doc-lint", "--root", t, "--write-baseline", "b.tsv", "a.md")
            self.assertEqual(open(os.path.join(t, "b.tsv")).read(), "a.md\tfiller\t2\n")


class Root(unittest.TestCase):
    def test_paths_resolve_from_plugin_root_not_cwd(self):
        code, out = run("doc-lint", os.path.join("test", "fixtures", "ok", "clean.py"), cwd="/")
        self.assertEqual(code, 0, out)

    def test_absolute_path_inside_root_accepted(self):
        code, out = run("doc-lint", os.path.join(ROOT, "test", "fixtures", "ok", "clean.py"))
        self.assertEqual(code, 0, out)

    def test_absolute_path_outside_root_refused(self):
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "x.md"), "# x\n")
            code, out = run("doc-lint", os.path.join(d, "x.md"))
        self.assertEqual(code, 2, out)

    def test_root_lints_repo_markdown_without_plugin_files(self):
        with tempfile.TemporaryDirectory() as d:
            write(os.path.join(d, "note.md"), "# x\n\n```agentflow-beta LR\n  a --> b\n```\n")
            code, out = run("doc-lint", "--root", d, "note.md")
            self.assertEqual(code, 0, out)
            code, out = run("doc-lint", "--root", d, "--all")
            self.assertEqual(code, 0, out)
            code, out = run("doc-lint", "--root", d, "../note.md")
            self.assertEqual(code, 2, out)
            code, out = run("doc-lint", "--root", d, os.path.join(d, "note.md"))
            self.assertEqual(code, 2, out)

    def test_all_is_clean_on_own_tree(self):
        code, out = run("doc-lint", "--all")
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
