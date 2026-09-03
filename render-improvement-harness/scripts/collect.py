#!/usr/bin/env python3
"""Gather every slide report's findings into findings.jsonl and print a short summary."""

from __future__ import annotations

import json
import re
import sys

import yaml

from common import DECKS, HARNESS


def main() -> None:
    rows = []
    for report in sorted(DECKS.glob("*/reports/*.md")):
        m = re.match(r"^---\n(.*?)\n---\n", report.read_text(), re.S)
        if not m:
            print(f"skip {report}: no frontmatter", file=sys.stderr)
            continue
        head = yaml.safe_load(m.group(1)) or {}
        for f in head.get("findings") or []:
            rows.append({"deck": head.get("deck", report.parent.parent.name), "slide": head.get("slide", int(report.stem)), "verdict": head.get("verdict"), "report": str(report.relative_to(HARNESS)), **f})
    out = HARNESS / "findings.jsonl"
    out.write_text("".join(json.dumps(r) + "\n" for r in rows))
    by_cat: dict[str, int] = {}
    for r in rows:
        by_cat[r.get("category", "?")] = by_cat.get(r.get("category", "?"), 0) + 1
    print(f"{len(rows)} finding(s) -> {out.relative_to(HARNESS.parent)}")
    for cat, n in sorted(by_cat.items(), key=lambda kv: -kv[1]):
        print(f"  {n:3d}  {cat}")


if __name__ == "__main__":
    main()
