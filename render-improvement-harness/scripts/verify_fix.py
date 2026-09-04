#!/usr/bin/env python3
"""Check one issue's fix: re-render its slides with a candidate engine, then compare against
the committed baseline and the LibreOffice reference.

Run it from the harness worktree, pointing at the worktree that holds the fix:

    .venv/bin/python render-improvement-harness/scripts/verify_fix.py <issue-id> \
        --engine ~/GitHub/fork/betteroffice.fix-pptx-font-fallback-style

The fix worktree is normally based on main and carries no harness, so the decks, the reference
renders and the baseline all live here while only the compiled engine comes from there.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import yaml
from PIL import Image, ImageDraw

from common import DECKS, ISSUES, ROOT, slide_name
from diff import metrics, verdict


def frontmatter(path: Path) -> dict:
    return yaml.safe_load(re.match(r"^---\n(.*?)\n---", path.read_text(), re.S).group(1))


def findings_by_slide() -> dict[tuple[str, str], list[tuple[str, str]]]:
    """Every issue's findings, keyed by the slide they sit on."""
    out: dict[tuple[str, str], list[tuple[str, str]]] = {}
    for report in sorted(ISSUES.glob("*/report.md")):
        head = frontmatter(report)
        for f in head.get("findings") or []:
            deck, num, _ = f.split("/")
            out.setdefault((deck, num), []).append((report.parent.name, head.get("category", "")))
    return out


def render_with(engine: Path, deck_id: str, slides: list[str], out_dir: Path) -> None:
    """Render the given slides using the engine compiled in `engine`, in a clean subprocess."""
    env = dict(os.environ)
    binding = engine / "bindings" / "python-pptx" / "python"
    if not (binding / "betteroffice_pptx").is_dir():
        sys.exit(f"no built binding at {binding}; run the worktree setup hook there first")
    env["PYTHONPATH"] = str(binding) + os.pathsep + env.get("PYTHONPATH", "")
    cmd = [sys.executable, str(Path(__file__).resolve()), "--_render", deck_id, str(out_dir), *slides]
    proc = subprocess.run(cmd, env=env, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.exit(f"render failed for {deck_id}:\n{proc.stdout}{proc.stderr}")
    print(proc.stdout.strip())


def render_worker(deck_id: str, out_dir: Path, slides: list[str]) -> None:
    import betteroffice_pptx as bo

    from render_bo import register_fonts

    origin = Path(bo.__file__).resolve()
    deck = bo.Presentation.open_path(str(DECKS / deck_id / "source.pptx"))
    register_fonts(deck)
    out_dir.mkdir(parents=True, exist_ok=True)
    for name in slides:
        png = deck.render_png(int(name) - 1, scale=1.0, background="slide")
        png.write(out_dir / f"{name}.png")
    print(f"rendered {len(slides)} slide(s) of {deck_id} with {origin}")


def triptych(ref: Path, before: Path, after: Path, dest: Path, labels: tuple[str, str, str]) -> None:
    images = [Image.open(p).convert("RGB") for p in (ref, before, after)]
    w, h = images[0].size
    images = [im if im.size == (w, h) else im.resize((w, h), Image.LANCZOS) for im in images]
    gap, top = 8, 22
    sheet = Image.new("RGB", (w * 3 + gap * 2, h + top), "white")
    draw = ImageDraw.Draw(sheet)
    for i, (im, label) in enumerate(zip(images, labels)):
        sheet.paste(im, (i * (w + gap), top))
        draw.text((i * (w + gap) + 4, 6), label, fill="black")
    sheet.save(dest)


def main() -> None:
    # The render worker re-enters this file in a subprocess with PYTHONPATH pointing at the
    # candidate engine, so it is dispatched before argparse sees the normal arguments.
    if "--_render" in sys.argv:
        deck_id, out_dir, *slides = sys.argv[sys.argv.index("--_render") + 1 :]
        return render_worker(deck_id, Path(out_dir), slides)

    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("issue_id")
    ap.add_argument("--engine", type=Path, default=ROOT, help="worktree holding the fix (default: this one)")
    args = ap.parse_args()

    folder = ISSUES / args.issue_id
    if not (folder / "report.md").exists():
        sys.exit(f"unknown issue {args.issue_id}")
    head = frontmatter(folder / "report.md")
    affected: dict[str, list[str]] = {}
    for f in head.get("findings") or []:
        deck, num, _ = f.split("/")
        affected.setdefault(deck, [])
        if num not in affected[deck]:
            affected[deck].append(num)

    others = findings_by_slide()
    out = folder / "verify"
    out.mkdir(exist_ok=True)
    engine = args.engine.expanduser().resolve()
    rows, worse, overlap_notes = [], [], []

    with tempfile.TemporaryDirectory() as tmp:
        for deck_id, slides in sorted(affected.items()):
            slides = sorted(slides)
            after_dir = Path(tmp) / deck_id
            render_with(engine, deck_id, slides, after_dir)
            for num in slides:
                ref = DECKS / deck_id / "lo-img" / f"{num}.png"
                before = DECKS / deck_id / "bo-img" / f"{num}.png"
                after = after_dir / f"{num}.png"
                if not (ref.exists() and before.exists() and after.exists()):
                    rows.append((f"{deck_id}/{num}", None, None, "missing render"))
                    continue
                b, _, _, _ = metrics(ref, before)
                a, _, _, _ = metrics(ref, after)
                delta = round(a["fine_pct"] - b["fine_pct"], 2)
                rows.append((f"{deck_id}/{num}", b, a, delta))
                if delta > 0.05:
                    worse.append(f"{deck_id}/{num}")
                triptych(ref, before, after, out / f"{deck_id}-{num}.png",
                         ("LibreOffice (reference, via pptx-pdf)", "BetterOffice before", "BetterOffice after"))
                co = {i for i, _ in others.get((deck_id, num), []) if i != args.issue_id}
                if co:
                    overlap_notes.append(f"  {deck_id}/{num}: also {', '.join(sorted(co))}")

    print(f"\nissue: {args.issue_id}   engine: {engine}")
    print(f"{'slide':28} {'before':>8} {'after':>8} {'delta':>8}   verdict")
    for name, b, a, delta in rows:
        if b is None:
            print(f"{name:28} {'-':>8} {'-':>8} {'-':>8}   {delta}")
            continue
        arrow = f"{b['verdict']} -> {a['verdict']}"
        print(f"{name:28} {b['fine_pct']:8.2f} {a['fine_pct']:8.2f} {delta:+8.2f}   {arrow}")

    if overlap_notes:
        print("\nOther issues have findings on these slides, so a residual is expected:")
        print("\n".join(overlap_notes))
    if worse:
        print(f"\nDiff rose on: {', '.join(worse)}")
        print("That is not automatically a regression. Check the issue's Verification section:")
        print("some fixes are documented to make a slide worse until a dependent issue lands.")

    summary = {
        "issue": args.issue_id,
        "engine": str(engine),
        "slides": [
            {"slide": n, "before": b, "after": a, "delta": d} for n, b, a, d in rows if b is not None
        ],
        "diff_rose_on": worse,
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"\ntriptychs and summary.json in {out.relative_to(ROOT)}/")
    print("Read the issue's Verification section before judging; the numbers alone do not decide it.")


if __name__ == "__main__":
    main()
