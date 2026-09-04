#!/usr/bin/env python3
"""Render issues/INDEX.md from clusters.json; issue report frontmatter is written back to clusters.json first."""

from __future__ import annotations

import json
import re

import yaml

from common import HARNESS, ISSUES

IMPACT = {"high": 0, "medium": 1, "low": 2}
EFFORT = {"easy": 0, "medium": 1, "hard": 2}


def main() -> None:
    path = HARNESS / "clusters.json"
    if not path.exists():
        raise SystemExit("clusters.json not found; run the failure-taxonomist first")
    data = json.loads(path.read_text())
    rows = []
    for c in data["clusters"]:
        report = ISSUES / c["id"] / "report.md"
        status, effort = "triaged", c.get("effort", "?")
        if report.exists():
            m = re.match(r"^---\n(.*?)\n---\n", report.read_text(), re.S)
            head = yaml.safe_load(m.group(1)) if m else {}
            status, effort = head.get("status", "investigated"), head.get("effort", effort)
            for key in ("effort", "confidence", "files"):
                if head.get(key):
                    c[key] = head[key]
        rows.append((c, status, effort))
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    rows.sort(key=lambda r: (IMPACT.get(r[0].get("impact"), 9), EFFORT.get(r[2], 9), -r[0].get("occurrences", 0)))
    lines = ["# Issues", "", f"Generated from `clusters.json` ({data.get('generated', '?')}). Ordered by impact, then effort.", "", "| # | issue | category | impact | effort | occurrences | decks | status |", "|---|---|---|---|---|---|---|---|"]
    for i, (c, status, effort) in enumerate(rows, 1):
        link = f"[{c['id']}]({c['id']}/report.md)" if (ISSUES / c["id"] / "report.md").exists() else f"`{c['id']}`"
        lines.append(f"| {i} | {link} | {c.get('category', '')} | {c.get('impact', '')} | {effort} | {c.get('occurrences', 0)} | {len(c.get('decks', []))} | {status} |")
    if data.get("deferred"):
        lines += ["", "## Deferred", ""] + [f"- `{d['finding']}`: {d['reason']}" for d in data["deferred"]]
    ISSUES.mkdir(exist_ok=True)
    (ISSUES / "INDEX.md").write_text("\n".join(lines) + "\n")
    print(f"{len(rows)} issue(s) -> issues/INDEX.md")


if __name__ == "__main__":
    main()
