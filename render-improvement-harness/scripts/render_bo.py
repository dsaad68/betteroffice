#!/usr/bin/env python3
"""Render the candidate images with betteroffice-pptx into decks/<id>/bo-img/NN.png."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import time

from common import CJK_DIR, FONT_DIR, SCALE, deck_dir, load_meta, save_meta, slide_name

FAMILIES = {
    "LiberationSans": ["Arial", "Helvetica", "Liberation Sans"],
    "LiberationSerif": ["Times New Roman", "Times", "Liberation Serif"],
    "LiberationMono": ["Courier New", "Courier", "Liberation Mono"],
    "Carlito": ["Calibri", "Carlito"],
    "Caladea": ["Cambria", "Caladea"],
}
STYLES = [("Regular", False, False), ("Bold", True, False), ("Italic", False, True), ("BoldItalic", True, True)]


def register_fonts(deck) -> int:
    faces = 0
    for stem, names in FAMILIES.items():
        for suffix, bold, italic in STYLES:
            path = FONT_DIR / f"{stem}-{suffix}.ttf"
            if not path.exists():
                continue
            data = path.read_bytes()
            for name in names:
                deck.register_font(name, data, bold=bold, italic=italic)
                faces += 1
    for path in sorted(CJK_DIR.glob("NotoSans*-Regular.otf")):
        data = path.read_bytes()
        for name in ("Noto Sans CJK", path.stem.split("-")[0]):
            deck.register_font(name, data)
            faces += 1
    return faces


def render(deck_id: str) -> None:
    try:
        import betteroffice_pptx as bo
    except ImportError:
        sys.exit("betteroffice_pptx not importable; run: cd bindings/python-pptx && maturin develop")
    d = deck_dir(deck_id)
    out = d / "bo-img"
    if out.exists():
        shutil.rmtree(out)
    out.mkdir()
    log: dict[str, dict] = {}
    try:
        deck = bo.Presentation.open_path(str(d / "source.pptx"))
    except Exception as error:  # noqa: BLE001
        log["open"] = {"error": f"{type(error).__name__}: {error}"}
        (d / "bo-log.json").write_text(json.dumps(log, indent=2) + "\n")
        sys.exit(f"{deck_id}: open failed: {error}")
    register_fonts(deck)
    for index in range(len(deck)):
        name = slide_name(index + 1)
        started = time.perf_counter()
        try:
            png = deck.render_png(index, scale=SCALE, background="slide")
        except Exception as error:  # noqa: BLE001
            log[name] = {"error": f"{type(error).__name__}: {error}"}
            print(f"{deck_id} slide {name}: {type(error).__name__}: {error}")
            continue
        png.write(out / f"{name}.png")
        log[name] = {
            "ms": round((time.perf_counter() - started) * 1000),
            "size": [png.width, png.height],
            "skipped_images": png.skipped_images,
        }
    (d / "bo-log.json").write_text(json.dumps(log, indent=2) + "\n")
    meta = load_meta(deck_id)
    meta["bo_slides"] = len(deck)
    meta["bo_failed"] = sorted(k for k, v in log.items() if "error" in v)
    save_meta(deck_id, meta)
    print(f"{deck_id}: {len(list(out.glob('*.png')))}/{len(deck)} BetterOffice image(s)")


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("deck_id")
    render(ap.parse_args().deck_id)
