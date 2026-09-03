"""Shared paths and helpers for the render improvement harness."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

HARNESS = Path(__file__).resolve().parent.parent
ROOT = HARNESS.parent
DECKS = HARNESS / "decks"
ISSUES = HARNESS / "issues"
FONT_DIR = ROOT / "packages" / "fonts" / "assets"
CJK_DIR = ROOT / "packages" / "fonts-cjk" / "assets"
PPTX_PDF = Path.home() / "GitHub" / "pptx-pdf" / "target" / "release" / "pptx-pdf"
DPI = 96
SCALE = 1.0


def slug(text: str) -> str:
    text = re.sub(r"\.pptx$", "", text, flags=re.I)
    text = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return text[:48].rstrip("-") or "deck"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def deck_dir(deck_id: str) -> Path:
    d = DECKS / deck_id
    if not (d / "source.pptx").exists():
        sys.exit(f"{deck_id}: no source.pptx under {d}; run add_deck.py first")
    return d


def load_meta(deck_id: str) -> dict:
    return json.loads((DECKS / deck_id / "meta.json").read_text())


def save_meta(deck_id: str, meta: dict) -> None:
    (DECKS / deck_id / "meta.json").write_text(json.dumps(meta, indent=2, sort_keys=True) + "\n")


def slide_name(n: int) -> str:
    return f"{n:02d}"
