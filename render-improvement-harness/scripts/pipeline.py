#!/usr/bin/env python3
"""Run the deterministic stages for one deck: register, render both sides, extract XML, diff."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

from common import DECKS, slug

SCRIPTS = Path(__file__).resolve().parent


def step(name: str, *args: str) -> None:
    print(f"== {name} {' '.join(args)}", flush=True)
    subprocess.run([sys.executable, str(SCRIPTS / name), *args], check=True)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("target", help="a .pptx path or an existing deck id")
    ap.add_argument("--id")
    ap.add_argument("--source-url", default="")
    ap.add_argument("--skip-lo", action="store_true", help="reuse existing LibreOffice renders")
    args = ap.parse_args()

    target = Path(args.target)
    if target.suffix.lower() == ".pptx" and target.exists():
        deck_id = args.id or slug(target.name)
        step("add_deck.py", str(target), "--id", deck_id, "--source-url", args.source_url)
    else:
        deck_id = args.target
        if not (DECKS / deck_id / "source.pptx").exists():
            sys.exit(f"unknown deck id {deck_id}")
    if not (args.skip_lo and (DECKS / deck_id / "lo-img").exists()):
        step("render_lo.py", deck_id)
    step("render_bo.py", deck_id)
    step("extract_xml.py", deck_id)
    step("diff.py", deck_id)
    (DECKS / deck_id / "reports").mkdir(exist_ok=True)
    print(f"done: {deck_id}")


if __name__ == "__main__":
    main()
