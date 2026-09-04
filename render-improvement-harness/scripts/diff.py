#!/usr/bin/env python3
"""Pixel-diff LibreOffice vs BetterOffice renders: heatmaps, side-by-side images, and diff-summary.json."""

from __future__ import annotations

import argparse
import json
import shutil

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

from common import deck_dir, load_meta, slide_name

THRESHOLD = 40
GRID = (4, 4)


def load(path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.int16)


def coarse(img: Image.Image) -> np.ndarray:
    small = img.convert("L").resize((64, 36), Image.BOX).filter(ImageFilter.GaussianBlur(1))
    return np.asarray(small, dtype=np.int16)


def verdict(coarse_pct: float, fine_pct: float) -> str:
    if fine_pct >= 7.0 or coarse_pct >= 4.0:
        return "major"
    if fine_pct < 1.5 and coarse_pct < 0.5:
        return "match"
    return "minor"


def metrics(lo_path, bo_path) -> tuple[dict, "Image.Image", "Image.Image", "np.ndarray"]:
    """Diff percentages for a reference/candidate pair, plus the images and mask for drawing."""
    lo_img = Image.open(lo_path).convert("RGB")
    bo_img = Image.open(bo_path).convert("RGB")
    sizes = {"lo_size": list(lo_img.size), "bo_size": list(bo_img.size)}
    if bo_img.size != lo_img.size:
        bo_img = bo_img.resize(lo_img.size, Image.LANCZOS)
    lo, bo = load(lo_path), np.asarray(bo_img, dtype=np.int16)
    mask = np.abs(lo - bo).max(axis=2) > THRESHOLD
    fine_pct = float(mask.mean() * 100)
    coarse_pct = float((np.abs(coarse(lo_img) - coarse(bo_img)) > THRESHOLD).mean() * 100)
    return {"fine_pct": round(fine_pct, 2), "coarse_pct": round(coarse_pct, 2), "verdict": verdict(coarse_pct, fine_pct), **sizes}, lo_img, bo_img, mask


def compare(lo_path, bo_path, heat_path, sbs_path) -> dict:
    stats, lo_img, bo_img, mask = metrics(lo_path, bo_path)
    fine_pct = stats["fine_pct"]
    h, w = mask.shape
    cells = []
    for gy in range(GRID[1]):
        for gx in range(GRID[0]):
            cell = mask[gy * h // GRID[1] : (gy + 1) * h // GRID[1], gx * w // GRID[0] : (gx + 1) * w // GRID[0]]
            cells.append({"cell": f"r{gy + 1}c{gx + 1}", "pct": round(float(cell.mean() * 100), 1)})
    cells.sort(key=lambda c: -c["pct"])

    base = (np.asarray(lo_img.convert("L"), dtype=np.float32) * 0.45 + 120).clip(0, 255).astype(np.uint8)
    heat = np.stack([base] * 3, axis=2)
    heat[mask] = [220, 30, 30]
    heat_img = Image.fromarray(heat)
    heat_img.save(heat_path)

    gap = 8
    sbs = Image.new("RGB", (w * 3 + gap * 2, h + 24), "white")
    for i, (img, label) in enumerate(((lo_img, "LibreOffice (reference)"), (bo_img, "BetterOffice (candidate)"), (heat_img, f"diff {fine_pct:.1f}%"))):
        sbs.paste(img, (i * (w + gap), 24))
        ImageDraw.Draw(sbs).text((i * (w + gap) + 4, 6), label, fill="black")
    sbs.save(sbs_path)
    return {
        "fine_pct": stats["fine_pct"],
        "coarse_pct": stats["coarse_pct"],
        "verdict": stats["verdict"],
        "hot_cells": cells[:4],
        "lo_size": stats["lo_size"],
        "bo_size": stats["bo_size"],
    }


def run(deck_id: str) -> None:
    d = deck_dir(deck_id)
    out = d / "diff-img"
    if out.exists():
        shutil.rmtree(out)
    out.mkdir()
    meta = load_meta(deck_id)
    summary = {}
    for n in range(1, meta["slides"] + 1):
        name = slide_name(n)
        lo, bo = d / "lo-img" / f"{name}.png", d / "bo-img" / f"{name}.png"
        if not lo.exists():
            summary[name] = {"verdict": "lo-missing"}
        elif not bo.exists():
            summary[name] = {"verdict": "bo-render-failed"}
        else:
            summary[name] = compare(lo, bo, out / f"{name}.png", out / f"{name}-sbs.png")
    (d / "diff-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    counts = {}
    for v in summary.values():
        counts[v["verdict"]] = counts.get(v["verdict"], 0) + 1
    print(f"{deck_id}: " + ", ".join(f"{k}={v}" for k, v in sorted(counts.items())))


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("deck_id")
    run(ap.parse_args().deck_id)
