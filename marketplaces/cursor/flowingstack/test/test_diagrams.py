import hashlib
import os
import re
import unittest

from lib import ROOT, read
from test_links import broken

PAGES = {
    "flow.md": ["flowchart"],
    "states.md": ["stateDiagram-v2"] * 4,
    "skills.md": ["graph"],
    "connections.md": ["flowchart"],
    "sequence.md": ["sequenceDiagram"],
    "install.md": ["flowchart"],
}
FENCE = re.compile(r"^```(mermaid|[a-zA-Z0-9-]+-beta)[^\n]*\n(.*?)^```", re.M | re.S)


def docs():
    yield os.path.join(ROOT, "README.md")
    for n in sorted(os.listdir(os.path.join(ROOT, "docs"))):
        if n.endswith(".md"):
            yield os.path.join(ROOT, "docs", n)


def blocks(path):
    return [(k, b) for k, b in FENCE.findall(read(path))]


def digest(body):
    return hashlib.sha256(body.encode()).hexdigest()[:16]


def known():
    rows = read(os.path.join(ROOT, "docs", "img", "rendered.tsv")).splitlines()
    return {r.split("\t")[0]: r.split("\t")[1] for r in rows if r and not r.startswith("#")}


class Pages(unittest.TestCase):
    def test_required_pages_hold_their_diagrams(self):
        for page, kinds in PAGES.items():
            with self.subTest(page):
                found = [b.split("\n", 2)[1].split()[0] for k, b in blocks(os.path.join(ROOT, "docs", page)) if k == "mermaid"]
                for kind in set(kinds):
                    self.assertGreaterEqual(found.count(kind), kinds.count(kind), found)

    def test_every_flowchart_or_graph_has_class_styling(self):
        for f in docs():
            for k, b in blocks(f):
                if k == "mermaid" and re.search(r"^(flowchart|graph)\b", b, re.M):
                    with self.subTest(os.path.relpath(f, ROOT)):
                        self.assertRegex(b, r"classDef \w+ fill:#")


class Rendered(unittest.TestCase):
    def test_every_block_rendered_or_known_good(self):
        k = known()
        for f in docs():
            for kind, b in blocks(f):
                with self.subTest(f"{os.path.relpath(f, ROOT)} {b[:40]!r}"):
                    self.assertIn(digest(b), k, "run scripts/render-diagrams")

    def test_rendered_images_exist_and_readme_links_them(self):
        readme = read(os.path.join(ROOT, "README.md"))
        for img in set(known().values()):
            if img == "known-good":
                continue
            with self.subTest(img):
                self.assertTrue(os.path.exists(os.path.join(ROOT, "docs", "img", img)))
        for page in PAGES:
            with self.subTest(page):
                self.assertIn(f"](docs/{page})", readme)

    def test_doc_links_resolve(self):
        self.assertEqual(broken("docs") + [b for b in broken(".") if b.startswith("README")], [])


class Harness(unittest.TestCase):
    def test_grok_bot_format_note(self):
        s = read(os.path.join(ROOT, "docs", "harness.md"))
        for w in ("Grok Bot", "Cursor", "--harness"):
            self.assertIn(w, s)


if __name__ == "__main__":
    unittest.main()
