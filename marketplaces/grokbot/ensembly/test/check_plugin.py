#!/usr/bin/env python3
"""Validate the ensembly Grok Bot plugin layout, skill frontmatter, and public-scan rules."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_PLUGIN_KEYS = {"name", "version", "description", "author", "license", "skills"}
SKILL_NAME_RE = re.compile(r"^name:\s*(\S+)\s*$", re.M)
SKILL_DESC_RE = re.compile(r"^description:\s*(.+)$", re.M)
HOST_PATTERNS = [
    (re.compile(r"/home/(?!box\b)[A-Za-z0-9._-]+"), "home path"),
    (re.compile(r"\b100\.\d{1,3}\.\d{1,3}\.\d{1,3}\b"), "tailnet address"),
    (re.compile(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", re.I), "machine id or uuid"),
]
MERMAID_FILL = "fill:#1e3a5f"
MERMAID_COLOR = "color:#ffffff"

def fail(msg: str) -> None:
    print(f"FAIL: {msg}")
    raise SystemExit(1)

def load_json(path: Path) -> dict:
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"{path}: invalid json ({exc})")
    if not isinstance(data, dict):
        fail(f"{path}: expected object")
    return data

def check_plugin_json(path: Path) -> dict:
    data = load_json(path)
    missing = REQUIRED_PLUGIN_KEYS - data.keys()
    if missing:
        fail(f"{path}: missing keys {sorted(missing)}")
    if data.get("name") != "ensembly":
        fail(f"{path}: name must be ensembly")
    if data.get("skills") != "./skills/":
        fail(f"{path}: skills must be ./skills/")
    author = data.get("author")
    if not isinstance(author, dict) or not author.get("name"):
        fail(f"{path}: author.name required")
    if "stoiber" in path.read_text().lower() or "2016" in json.dumps(data):
        # year in version-adjacent metadata is fine; 2016 leftover copyright is not in this file
        pass
    return data

def skill_dirs() -> list[Path]:
    skills = ROOT / "skills"
    found = sorted(p for p in skills.iterdir() if p.is_dir())
    if not found:
        fail("no skill directories")
    return found

def check_skill(path: Path) -> None:
    text = (path / "SKILL.md").read_text() if (path / "SKILL.md").exists() else ""
    if not text:
        fail(f"{path}/SKILL.md missing")
    if not text.startswith("---"):
        fail(f"{path.name}: missing frontmatter")
    name = SKILL_NAME_RE.search(text)
    desc = SKILL_DESC_RE.search(text)
    if not name:
        fail(f"{path.name}: missing name")
    if name.group(1) != path.name:
        fail(f"{path.name}: folder and name: {name.group(1)} differ")
    if not desc:
        fail(f"{path.name}: missing description")
    if "```mermaid" not in text:
        fail(f"{path.name}: README-style diagram required")
    if MERMAID_FILL not in text or MERMAID_COLOR not in text:
        fail(f"{path.name}: mermaid must use dark fill and white text")
    if "mcp.json" in text or ".mcp.json" in text:
        fail(f"{path.name}: this plugin must not add an MCP server")

def scan_public(path: Path) -> None:
    if path.suffix.lower() not in {".md", ".json", ".py", ".txt"} and path.name != "LICENSE":
        return
    text = path.read_text()
    rel = path.relative_to(ROOT)
    if path.name == "check_plugin.py":
        return
    leftover = ("Maximilian " + "Stoiber") in text or ("Copyright (c) " + "2016") in text
    if leftover:
        fail(f"{rel}: leftover 2016 boilerplate license")
    for pat, label in HOST_PATTERNS:
        if path.name == "check_plugin.py":
            continue
        hit = pat.search(text)
        if hit:
            fail(f"{rel}: {label} leaked: {hit.group(0)}")

def main() -> None:
    plugin = check_plugin_json(ROOT / ".cursor-plugin/plugin.json")
    root_plugin = check_plugin_json(ROOT / "plugin.json")
    if plugin != root_plugin:
        fail("plugin.json and .cursor-plugin/plugin.json differ")
    expected = {"ensembly", "record-layer", "verified-retrieve"}
    found = {p.name for p in skill_dirs()}
    if found != expected:
        fail(f"skills {sorted(found)} != {sorted(expected)}")
    for path in skill_dirs():
        check_skill(path)
    readme = (ROOT / "README.md").read_text()
    if MERMAID_FILL not in readme or MERMAID_COLOR not in readme:
        fail("README mermaid must use dark fill and white text")
    if not (ROOT / "LICENSE").exists():
        fail("LICENSE missing")
    license_text = (ROOT / "LICENSE").read_text()
    if "Copyright (c) 2026 thecuriousts" not in license_text:
        fail("LICENSE must be MIT under thecuriousts, 2026")
    for path in ROOT.rglob("*"):
        if path.is_file():
            scan_public(path)
    print("OK: plugin.json, 3 skills, mermaid contrast, LICENSE, public scan")

if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as exc:
        fail(str(exc))
