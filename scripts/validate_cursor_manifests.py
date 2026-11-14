#!/usr/bin/env python3
"""Validate committed Cursor plugin and marketplace manifests against vendored schemas."""
from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    import jsonschema
except ImportError as exc:
    print("FAIL: install jsonschema (e.g. pip install jsonschema)", file=sys.stderr)
    raise SystemExit(2) from exc

REPO = Path(__file__).resolve().parents[1]
SCHEMA_DIR = Path(__file__).resolve().parent / "cursor-plugin-schemas"
PLUGIN_SCHEMA = json.loads((SCHEMA_DIR / "plugin.schema.json").read_text())
MARKETPLACE_SCHEMA = json.loads((SCHEMA_DIR / "marketplace.schema.json").read_text())

MANIFESTS = [
    REPO / ".cursor-plugin/marketplace.json",
    REPO / "marketplaces/cursor/flowingstack/.cursor-plugin/plugin.json",
    REPO / "marketplaces/grokbot/ensembly/.cursor-plugin/plugin.json",
]


def validate(path: Path, schema: dict) -> None:
    data = json.loads(path.read_text())
    jsonschema.validate(data, schema)
    print(f"OK {path.relative_to(REPO)}")


def main() -> None:
    validate(REPO / ".cursor-plugin/marketplace.json", MARKETPLACE_SCHEMA)
    for path in MANIFESTS[1:]:
        validate(path, PLUGIN_SCHEMA)
    print("OK: all manifests")


if __name__ == "__main__":
    try:
        main()
    except jsonschema.ValidationError as exc:
        print(f"FAIL: {exc.message}", file=sys.stderr)
        raise SystemExit(1) from exc
    except (OSError, json.JSONDecodeError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc