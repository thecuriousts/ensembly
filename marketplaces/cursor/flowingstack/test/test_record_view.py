import json
import os
import subprocess
import sys
import tempfile
import unittest

from lib import FIX, ROOT

EXPORT = os.path.join(FIX, "record-view", "e1-replay-001.export.json")


def view(path):
    p = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", "record-view"), path],
                       capture_output=True, text=True,
                       env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
    return p.returncode, p.stdout, p.stderr


def tmp_json(doc):
    f = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
    json.dump(doc, f)
    f.close()
    return f.name


class RecordView(unittest.TestCase):
    def test_shows_task_result_tokens_and_commit(self):
        code, out, _ = view(EXPORT)
        self.assertEqual(code, 0)
        lines = out.splitlines()
        self.assertEqual(lines[0], "export evalbench-export-v1")
        self.assertEqual(lines[1], "commit a0b5c4d16f377fd5ad05a357d213b1bb7cbe4e81")
        self.assertTrue(lines[2].startswith("pulse pack_sha256 "))
        self.assertIn("run replay-001 dataset 55faad2e303c22039226da1295312218c6f3b51af4d643d00ab5af8364b4b3df "
                      "tasks 12 first_pass 7 score 58 median_tokens 421 models synthetic-replay-model", lines)
        self.assertIn("task t01-full pass tokens 422 in 410 out 12 model synthetic-replay-model prompt gaps-v1", lines)
        self.assertIn("task t06-substring-trap fail tokens 401 in 360 out 41 model synthetic-replay-model prompt gaps-v1",
                      lines)
        self.assertEqual(sum(1 for l in lines if l.startswith("task ")), 12)

    def test_a_stated_score_that_disagrees_with_the_rows_exits_1(self):
        doc = json.load(open(EXPORT))
        doc["runs"][0]["first_pass"] = 12
        code, _, err = view(tmp_json(doc))
        self.assertEqual(code, 1)
        self.assertIn("disagrees", err)

    def test_not_an_export_exits_2(self):
        for doc in ({"format": "pulse-pack-v1"}, {"format": "evalbench-export-v1", "commit": "abc", "runs": []}):
            with self.subTest(doc):
                self.assertEqual(view(tmp_json(doc))[0], 2)
        self.assertEqual(view(os.path.join(FIX, "record-view", "missing.json"))[0], 2)

    def test_reads_only(self):
        before = open(EXPORT, "rb").read()
        view(EXPORT)
        self.assertEqual(open(EXPORT, "rb").read(), before)


if __name__ == "__main__":
    unittest.main()
