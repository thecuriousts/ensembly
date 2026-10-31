import os
import re
import unittest

from lib import ROOT, read

KINDS = {"Flow": "flowchart", "Stages": "stateDiagram-v2", "Ledger": "sequenceDiagram"}


class Readme(unittest.TestCase):
    def test_flow_stages_ledger_diagrams(self):
        s = read(os.path.join(ROOT, "README.md"))
        for h, kind in KINDS.items():
            with self.subTest(h):
                m = re.search(rf"^## {h}\n\n```mermaid\n(%%\{{init:.*theme.*\}}%%)\n{kind}\b", s, re.M)
                self.assertIsNotNone(m)


if __name__ == "__main__":
    unittest.main()
