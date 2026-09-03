#!/usr/bin/env python3
"""Gather every slide report's findings into findings.jsonl and print a short summary."""

from __future__ import annotations

import json
import re
import sys

import yaml

from common import DECKS, HARNESS

QUOTE_KEYS = ("summary", "xml", "shape")


def frontmatter(text: str) -> str | None:
    if text.startswith("---\n"):
        m = re.match(r"^---\n(.*?)\n---\n", text, re.S)
        return m.group(1) if m else None
    if text.startswith("deck:"):
        return text.split("\n---\n", 1)[0]
    return None


def quote_values(block: str) -> str:
    """Wrap free-text values in double quotes so stray quotes and colons parse."""
    out = []
    for line in block.split("\n"):
        m = re.match(r"^(\s*(?:- )?)(%s): (.*)$" % "|".join(QUOTE_KEYS), line)
        if m and not re.fullmatch(r'"(?:[^"\\]|\\.)*"', m.group(3)):
            value = m.group(3).replace("\\", "\\\\").replace('"', '\\"')
            line = f'{m.group(1)}{m.group(2)}: "{value}"'
        out.append(line)
    return "\n".join(out)


def parse(block: str) -> dict:
    try:
        return yaml.safe_load(block) or {}
    except yaml.YAMLError:
        return yaml.safe_load(quote_values(block)) or {}


def main() -> None:
    rows = []
    for report in sorted(DECKS.glob("*/reports/*.md")):
        block = frontmatter(report.read_text())
        if block is None:
            print(f"skip {report}: no frontmatter", file=sys.stderr)
            continue
        try:
            head = parse(block)
        except yaml.YAMLError as error:
            print(f"skip {report}: {str(error).splitlines()[0]}", file=sys.stderr)
            continue
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
