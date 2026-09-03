#!/usr/bin/env python3
"""Register a deck: copy it to decks/<id>/source.pptx and write meta.json."""

from __future__ import annotations

import argparse
import shutil
import zipfile
from pathlib import Path

from common import DECKS, save_meta, sha256, slug


def slide_count(path: Path) -> int:
    with zipfile.ZipFile(path) as z:
        return sum(1 for n in z.namelist() if n.startswith("ppt/slides/slide") and n.endswith(".xml"))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("pptx", type=Path)
    ap.add_argument("--id", help="deck id (default: slug of the file name)")
    ap.add_argument("--source-url", default="", help="where the deck came from")
    args = ap.parse_args()

    deck_id = args.id or slug(args.pptx.name)
    target = DECKS / deck_id
    target.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args.pptx, target / "source.pptx")
    meta = {
        "id": deck_id,
        "original_name": args.pptx.name,
        "source_url": args.source_url,
        "sha256": sha256(args.pptx),
        "slides": slide_count(args.pptx),
        "size_bytes": args.pptx.stat().st_size,
    }
    save_meta(deck_id, meta)
    print(f"{deck_id}: {meta['slides']} slide(s) -> {target.relative_to(DECKS.parent.parent)}")


if __name__ == "__main__":
    main()
