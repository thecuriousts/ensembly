#!/usr/bin/env python3
"""Validate the ensembly Grok Bot plugin layout, skill frontmatter, and public-scan rules."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parents[2]
CATALOG = REPO / ".cursor-plugin/marketplace.json"
ROW_SOURCE = "marketplaces/grokbot/ensembly"
RED_FIXTURES = {"test/fixtures/catalog-leak.json"}
REQUIRED_PLUGIN_KEYS = {"name", "version", "description", "author", "license", "skills"}
HOST_PATTERNS = [
    (re.compile(r"/home/(?!box\b)[A-Za-z0-9._-]+"), "home path"),
    (re.compile(r"/Users/[A-Za-z0-9._-]+"), "home path"),
    (re.compile(r"\b100\.\d{1,3}\.\d{1,3}\.\d{1,3}\b"), "tailnet address"),
    (re.compile(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", re.I), "machine id or uuid"),
    (re.compile(r"\b[\w-]+(?:\.[\w-]+)*\.ts\.net\b", re.I), "tailnet hostname"),
    (re.compile(r"\b[\w-]+\.local\b", re.I), "mdns hostname"),
    (re.compile(r"[\w.+-]+@(?!users\.noreply\.github\.com\b|example\.(?:com|org)\b)[\w-]+(?:\.[\w-]+)+"), "email"),
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
    return data

def frontmatter(text: str) -> dict[str, str]:
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        return {}
    try:
        end = lines.index("---", 1)
    except ValueError:
        return {}
    fields: dict[str, str] = {}
    body = lines[1:end]
    i = 0
    while i < len(body):
        m = re.match(r"^([A-Za-z_][\w-]*):\s*(.*)$", body[i])
        i += 1
        if not m:
            continue
        key, value = m.group(1), m.group(2).strip()
        if re.fullmatch(r"[>|][+-]?", value):
            block = []
            while i < len(body) and (not body[i].strip() or body[i][:1] in " \t"):
                block.append(body[i].strip())
                i += 1
            joiner = " " if value[0] == ">" else "\n"
            value = joiner.join(line for line in block if line)
        fields[key] = value.strip().strip("\"'")
    return fields

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
    fields = frontmatter(text)
    name = fields.get("name", "")
    desc = fields.get("description", "")
    if not name:
        fail(f"{path.name}: missing name")
    if name != path.name:
        fail(f"{path.name}: folder and name: {name} differ")
    if len(desc) < 20:
        fail(f"{path.name}: missing description")
    if "```mermaid" not in text:
        fail(f"{path.name}: README-style diagram required")
    if MERMAID_FILL not in text or MERMAID_COLOR not in text:
        fail(f"{path.name}: mermaid must use dark fill and white text")
    if "mcp.json" in text or ".mcp.json" in text:
        fail(f"{path.name}: this plugin must not add an MCP server")

def deny_patterns() -> list:
    """Real host names, one per line, from PRIVACY_DENY_FILE kept outside the repo."""
    path = os.environ.get("PRIVACY_DENY_FILE")
    if not path or not os.path.exists(path):
        return []
    rows = [line.strip() for line in open(path) if line.strip()]
    return [(re.compile(r"(?i)\b" + re.escape(row) + r"\b"), "deny-list host") for row in rows]

def scan_text(rel, text: str) -> None:
    for pat, label in HOST_PATTERNS + deny_patterns():
        hit = pat.search(text)
        if hit:
            shown = "<deny-list>" if label == "deny-list host" else hit.group(0)
            fail(f"{rel}: {label} leaked: {shown}")

def scan_public(path: Path) -> None:
    if path.suffix.lower() not in {".md", ".json", ".py", ".txt"} and path.name != "LICENSE":
        return
    text = path.read_text()
    rel = path.relative_to(ROOT)
    if path.name == "check_plugin.py" or rel.as_posix() in RED_FIXTURES:
        return
    leftover = ("Maximilian " + "Stoiber") in text or ("Copyright (c) " + "2016") in text
    if leftover:
        fail(f"{rel}: leftover 2016 boilerplate license")
    scan_text(rel, text)

def check_catalog(path: Path = CATALOG) -> None:
    """The marketplace catalog is public too: one ensembly row, whole file scanned."""
    data = load_json(path)
    rows = [p for p in data.get("plugins", []) if isinstance(p, dict) and p.get("source") == ROW_SOURCE]
    if len(rows) != 1 or rows[0].get("name") != "ensembly":
        fail(f"{path.name}: need one ensembly row at {ROW_SOURCE}")
    scan_text(path.name, path.read_text())

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
    check_catalog()
    print("OK: plugin.json, 3 skills, mermaid contrast, LICENSE, public scan, catalog scan")

if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as exc:
        fail(str(exc))
