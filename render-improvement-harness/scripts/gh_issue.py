#!/usr/bin/env python3
"""Render a GitHub issue body for one investigated cluster from templates/github-issue.md; --create files it."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

import yaml

from common import DECKS, HARNESS, ISSUES, ROOT, load_meta

UPSTREAM = "openooxml/betteroffice"
GIST = "https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0"
PPTX_PDF = "https://github.com/dsaad68/pptx-pdf"
MAPPING = ISSUES / "github-issues.json"
PATH_REF = re.compile(r"`?((?:crates|packages|bindings)/[\w./-]+\.(?:rs|ts|tsx|toml|md)):(\d+)(?:-(\d+))?`?")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()


def fork_slug() -> str:
    url = git("remote", "get-url", "fork")
    return re.sub(r"^.*github\.com[:/]|\.git$", "", url)


def frontmatter(text: str) -> tuple[dict, str]:
    m = re.match(r"^---\n(.*?)\n---\n(.*)$", text, re.S)
    return yaml.safe_load(m.group(1)), m.group(2)


def sections(body: str) -> dict[str, str]:
    out, key, buf = {}, "_pre", []
    for line in body.split("\n"):
        if line.startswith("## "):
            out[key] = "\n".join(buf).strip()
            key, buf = line[3:].strip(), []
        else:
            buf.append(line)
    out[key] = "\n".join(buf).strip()
    return out


def pick(secs: dict[str, str], *prefixes: str) -> tuple[str, str]:
    for k in secs:
        if any(k.lower().startswith(p) for p in prefixes):
            return k, secs[k]
    return "", ""


class Linker:
    def __init__(self) -> None:
        self.base = git("merge-base", "HEAD", "main")
        self.head = git("rev-parse", "HEAD")
        self.fork = fork_slug()
        self.branch = git("rev-parse", "--abbrev-ref", "HEAD")
        self.changed = set(git("diff", "--name-only", self.base, "HEAD").split("\n"))

    def link(self, m: re.Match) -> str:
        path, start, end = m.group(1), m.group(2), m.group(3)
        anchor = f"#L{start}" + (f"-L{end}" if end else "")
        repo, sha = (self.fork, self.head) if path in self.changed else (UPSTREAM, self.base)
        label = f"{path}:{start}" + (f"-{end}" if end else "")
        return f"[`{label}`](https://github.com/{repo}/blob/{sha}/{path}{anchor})"

    def linkify(self, text: str) -> str:
        return PATH_REF.sub(self.link, text)

    def raw(self, issue_id: str, name: str) -> str:
        return f"https://raw.githubusercontent.com/{self.fork}/{self.branch}/render-improvement-harness/issues/{issue_id}/{name}"

    def harness(self) -> str:
        return f"https://github.com/{self.fork}/tree/{self.branch}/render-improvement-harness"

    def report(self, issue_id: str) -> str:
        return f"https://github.com/{self.fork}/blob/{self.branch}/render-improvement-harness/issues/{issue_id}/report.md"


def evidence_block(secs: dict[str, str], issue_id: str, lk: Linker, folder, only: set[int] | None = None) -> str:
    captions = {}
    for row in secs.get("Evidence", "").split("\n"):
        cells = [c.strip() for c in row.strip("|").split("|")]
        if len(cells) >= 3 and cells[0].isdigit():
            captions[int(cells[0])] = (cells[1], cells[2])
    parts = []
    for png in sorted(folder.glob("evidence-*.png"), key=lambda p: int(re.search(r"(\d+)", p.stem).group(1))):
        n = int(re.search(r"(\d+)", png.stem).group(1))
        if only and n not in only:
            continue
        where, what = captions.get(n, ("", ""))
        caption = f"**{n}. {where}** {what}".strip() if where else f"**{n}.**"
        parts.append(f"{caption}\n\n![{png.stem}]({lk.raw(issue_id, png.name)})")
    return "\n\n".join(parts)


def drop_evidence_refs(text: str, dropped: set[int]) -> str:
    """Remove prose citations of evidence images that are not embedded, then tidy the punctuation."""
    for n in dropped:
        text = re.sub(r"\s*\bevidence-%d\.png\b" % n, "", text)
    text = re.sub(r"\(\s*(?:,\s*)+", "(", text)
    text = re.sub(r"(?:,\s*)+\)", ")", text)
    text = re.sub(r"\s*\(\s*\)", "", text)
    return re.sub(r"[ \t]{2,}", " ", text)


def repro_decks(head: dict) -> tuple[str, int]:
    per_deck: dict[str, set] = {}
    for f in head.get("findings", []):
        deck, slide, _ = f.split("/")
        per_deck.setdefault(deck, set()).add(int(slide))
    lines, first_index = [], 0
    for deck, slides in sorted(per_deck.items()):
        meta = load_meta(deck)
        nums = ", ".join(str(s) for s in sorted(slides))
        lines.append(f"- `{meta['original_name']}` ([source]({meta['source_url']})), slide{'s' if len(slides) > 1 else ''} {nums}")
        if not first_index:
            first_index = min(slides) - 1
    return "\n".join(lines), first_index


def related(issue_id: str, text: str, mapping: dict) -> str:
    ids = [p.name for p in ISSUES.iterdir() if p.is_dir() and p.name != issue_id]
    hits = sorted(i for i in ids if re.search(r"\b" + re.escape(i) + r"\b", text))
    if not hits:
        return "none."
    return ", ".join(f"#{mapping[i]['number']}" if i in mapping else f"`{i}`" for i in hits)


def render(issue_id: str, only: set[int] | None = None) -> tuple[str, str]:
    folder = ISSUES / issue_id
    head, body = frontmatter((folder / "report.md").read_text())
    _, sol_body = (None, (folder / "possible-solution.md").read_text())
    secs, sol = sections(body), sections(sol_body)
    lk = Linker()
    mapping = json.loads(MAPPING.read_text()) if MAPPING.exists() else {}
    root_key, root = pick(secs, "root cause")
    if only:
        dropped = {int(re.search(r"(\d+)", q.stem).group(1)) for q in folder.glob("evidence-*.png")} - only
        secs = {k: drop_evidence_refs(v, dropped) for k, v in secs.items()}
        sol = {k: drop_evidence_refs(v, dropped) for k, v in sol.items()}
        root = drop_evidence_refs(root, dropped)
    known = {"symptom", "evidence", "verification", root_key.lower()}
    extra = "\n\n".join(f"*{k}*\n\n{v}" for k, v in secs.items() if k.lower() not in known and k != "_pre" and v)
    sketch = sol.get("Sketch", "")
    if not sketch:
        sketch = "\n\n".join(f"*{k}*\n\n{v}" for k, v in sol.items() if k not in ("Approach", "Risks", "Effort", "_pre"))
    decks_md, first_index = repro_decks(head)
    occurrences = head.get("occurrences", len(head.get("findings", [])))
    deck_count = len(head.get("decks", []))
    clusters = {c["id"]: c for c in json.loads((HARNESS / "clusters.json").read_text())["clusters"]}
    fields = {
        "title": clusters.get(issue_id, {}).get("title") or head["title"],
        "symptom": lk.linkify(secs.get("Symptom", "")),
        "occurrences": occurrences,
        "occurrences_plural": "" if occurrences == 1 else "s",
        "deck_count": deck_count,
        "deck_count_plural": "" if deck_count == 1 else "s",
        "impact": head.get("impact", "?"),
        "effort": head.get("effort", "?"),
        "confidence": head.get("confidence", "?"),
        "evidence": evidence_block(secs, issue_id, lk, folder, only),
        "repro_decks": decks_md,
        "repro_slide_index": first_index,
        "expected": "PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.",
        "root_cause": lk.linkify(root) + ("" if "confirmed" in root_key.lower() else "\n\n_(hypothesis, not yet confirmed by a fix)_"),
        "approach": lk.linkify(sol.get("Approach", "")),
        "sketch": lk.linkify(sketch),
        "risks": lk.linkify(sol.get("Risks", "")),
        "verification": lk.linkify(secs.get("Verification", "")),
        "extra_sections": lk.linkify(extra) or "none.",
        "related": related(issue_id, body + sol_body, mapping),
        "files": ", ".join(f"`{f}`" for f in head.get("files", [])),
        "report_link": lk.report(issue_id),
        "gist_link": GIST,
        "harness_link": lk.harness(),
        "pptx_pdf_link": PPTX_PDF,
        "cluster_count": len(clusters),
    }
    template = (HARNESS / "templates" / "github-issue.md").read_text()
    out = re.sub(r"\{\{(\w+)\}\}", lambda m: str(fields[m.group(1)]), template)
    title = re.search(r"<!-- title: (.*?) -->", out).group(1)
    out = re.sub(r"<!--.*?-->\n?", "", out, flags=re.S).strip() + "\n"
    return title, out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("issue_id")
    ap.add_argument("--create", action="store_true", help="file it on GitHub with gh and record the number")
    ap.add_argument("--repo", default=UPSTREAM)
    ap.add_argument("--evidence", help="comma-separated evidence numbers to embed (default: all)")
    args = ap.parse_args()
    only = {int(n) for n in args.evidence.split(",")} if args.evidence else None
    title, body = render(args.issue_id, only)
    out = ISSUES / args.issue_id / "github-issue.md"
    out.write_text(f"# {title}\n\n{body}")
    print(f"{args.issue_id}: {len(body):,} chars -> {out.relative_to(ROOT)}")
    if not args.create:
        return
    mapping = json.loads(MAPPING.read_text()) if MAPPING.exists() else {}
    if args.issue_id in mapping:
        sys.exit(f"already filed as {mapping[args.issue_id]['url']}")
    url = subprocess.run(["gh", "issue", "create", "--repo", args.repo, "--title", title, "--label", "bug", "--body", body], capture_output=True, text=True, check=True).stdout.strip()
    mapping[args.issue_id] = {"url": url, "number": int(url.rsplit("/", 1)[1])}
    MAPPING.write_text(json.dumps(mapping, indent=2, sort_keys=True) + "\n")
    print(url)


if __name__ == "__main__":
    main()
