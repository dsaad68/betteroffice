#!/usr/bin/env python3
"""Extract per-slide XML plus its layout, master, theme, charts and a shape summary into decks/<id>/xml/NN/."""

from __future__ import annotations

import argparse
import json
import posixpath
import re
import shutil
import xml.etree.ElementTree as ET
import zipfile

from common import deck_dir, slide_name

NS = {
    "p": "http://schemas.openxmlformats.org/presentationml/2006/main",
    "a": "http://schemas.openxmlformats.org/drawingml/2006/main",
    "r": "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
    "rel": "http://schemas.openxmlformats.org/package/2006/relationships",
}
R_ID = "{%s}id" % NS["r"]


def rels_of(z: zipfile.ZipFile, part: str) -> dict[str, tuple[str, str]]:
    base, name = posixpath.split(part)
    rels_path = posixpath.join(base, "_rels", name + ".rels")
    if rels_path not in z.namelist():
        return {}
    out = {}
    for rel in ET.fromstring(z.read(rels_path)).findall("rel:Relationship", NS):
        target = rel.get("Target")
        if rel.get("TargetMode") != "External":
            target = posixpath.normpath(posixpath.join(base, target))
        out[rel.get("Id")] = (rel.get("Type").rsplit("/", 1)[-1], target)
    return out


def first_of(rels: dict, kind: str) -> str | None:
    return next((t for k, t in rels.values() if k == kind), None)


def emu_box(xfrm, sz, tf) -> dict | None:
    if xfrm is None:
        return None
    off, ext = xfrm.find("a:off", NS), xfrm.find("a:ext", NS)
    if off is None or ext is None:
        return None
    x, y, cx, cy = tf(int(off.get("x")), int(off.get("y")), int(ext.get("cx")), int(ext.get("cy")))
    w, h = sz
    box = {"emu": [round(x), round(y), round(cx), round(cy)], "frac": [round(x / w, 3), round(y / h, 3), round((x + cx) / w, 3), round((y + cy) / h, 3)]}
    if xfrm.get("rot"):
        box["rot"] = int(xfrm.get("rot")) / 60000
    if xfrm.get("flipH"):
        box["flipH"] = True
    if xfrm.get("flipV"):
        box["flipV"] = True
    return box


def group_transform(xfrm, tf):
    """Compose the parent transform with a group's child-space mapping."""
    if xfrm is None:
        return tf
    off, ext, choff, chext = (xfrm.find(q, NS) for q in ("a:off", "a:ext", "a:chOff", "a:chExt"))
    if None in (off, ext, choff, chext):
        return tf
    ox, oy, ex, ey = int(off.get("x")), int(off.get("y")), int(ext.get("cx")), int(ext.get("cy"))
    cx0, cy0, cw, ch = int(choff.get("x")), int(choff.get("y")), int(chext.get("cx")), int(chext.get("cy"))
    sx, sy = (ex / cw if cw else 1.0), (ey / ch if ch else 1.0)

    def inner(x, y, w, h):
        return tf(ox + (x - cx0) * sx, oy + (y - cy0) * sy, w * sx, h * sy)

    return inner


def text_of(el) -> str:
    return "".join(t.text or "" for t in el.iter("{%s}t" % NS["a"]))[:80]


def shapes(tree, sz, depth=0, tf=lambda x, y, w, h: (x, y, w, h)) -> list[dict]:
    out = []
    for el in tree.find("p:cSld/p:spTree", NS) if depth == 0 else tree:
        tag = el.tag.split("}")[1]
        if tag not in ("sp", "pic", "graphicFrame", "grpSp", "cxnSp"):
            continue
        nv = el.find("./*/p:cNvPr", NS)
        entry = {"kind": tag, "id": nv.get("id") if nv is not None else None, "name": nv.get("name") if nv is not None else None, "depth": depth}
        if nv is not None and nv.get("hidden") == "1":
            entry["hidden"] = True
        ph = el.find("./*/p:nvPr/p:ph", NS)
        if ph is not None:
            entry["placeholder"] = {k: v for k, v in ph.attrib.items()}
        xfrm = next((x for x in (el.find(q, NS) for q in ("p:spPr/a:xfrm", "p:xfrm", "p:grpSpPr/a:xfrm")) if x is not None), None)
        box = emu_box(xfrm, sz, tf)
        if box:
            entry["box"] = box
        geom = el.find("p:spPr/a:prstGeom", NS)
        if geom is not None:
            entry["preset"] = geom.get("prst")
        elif el.find("p:spPr/a:custGeom", NS) is not None:
            entry["preset"] = "custGeom"
        graphic = el.find("a:graphic/a:graphicData", NS)
        if graphic is not None:
            entry["graphic"] = graphic.get("uri", "").rsplit("/", 1)[-1]
        txt = text_of(el) if tag == "sp" else ""
        if txt.strip():
            entry["text"] = txt.strip()
        if tag == "grpSp":
            entry["children"] = shapes(el, sz, depth + 1, group_transform(xfrm, tf))
        out.append(entry)
    return out


def extract(deck_id: str) -> None:
    d = deck_dir(deck_id)
    out_root = d / "xml"
    if out_root.exists():
        shutil.rmtree(out_root)
    out_root.mkdir()
    with zipfile.ZipFile(d / "source.pptx") as z:
        pres = ET.fromstring(z.read("ppt/presentation.xml"))
        sz_el = pres.find("p:sldSz", NS)
        sz = (int(sz_el.get("cx")), int(sz_el.get("cy")))
        pres_rels = rels_of(z, "ppt/presentation.xml")
        (out_root / "presentation.xml").write_bytes(z.read("ppt/presentation.xml"))
        index = {"slide_size_emu": list(sz), "slides": []}
        for n, sld in enumerate(pres.find("p:sldIdLst", NS).findall("p:sldId", NS), start=1):
            slide_part = pres_rels[sld.get(R_ID)][1]
            slide_rels = rels_of(z, slide_part)
            layout_part = first_of(slide_rels, "slideLayout")
            layout_rels = rels_of(z, layout_part) if layout_part else {}
            master_part = first_of(layout_rels, "slideMaster")
            master_rels = rels_of(z, master_part) if master_part else {}
            theme_part = first_of(master_rels, "theme")
            out = out_root / slide_name(n)
            out.mkdir()
            for label, part in (("slide", slide_part), ("layout", layout_part), ("master", master_part), ("theme", theme_part)):
                if part:
                    (out / f"{label}.xml").write_bytes(z.read(part))
            rels_dump = {rid: {"type": k, "target": t} for rid, (k, t) in slide_rels.items()}
            (out / "slide.rels.json").write_text(json.dumps(rels_dump, indent=2) + "\n")
            extra = []
            for rid, (kind, target) in slide_rels.items():
                if kind in ("chart", "diagramData", "diagramDrawing", "diagramLayout", "oleObject") and target in z.namelist():
                    name = f"{kind}-{posixpath.basename(target)}"
                    (out / name).write_bytes(z.read(target))
                    extra.append(name)
                    if kind == "chart":
                        for _, (ck, ct) in rels_of(z, target).items():
                            if ck in ("chartStyle", "chartColorStyle", "chartUserShapes") and ct in z.namelist():
                                (out / f"{ck}-{posixpath.basename(ct)}").write_bytes(z.read(ct))
            slide_tree = ET.fromstring(z.read(slide_part))
            hidden = slide_tree.get("show") == "0"
            summary = {
                "slide": n,
                "part": slide_part,
                "layout": layout_part,
                "master": master_part,
                "theme": theme_part,
                "hidden": hidden,
                "has_background": slide_tree.find("p:cSld/p:bg", NS) is not None,
                "extra_parts": extra,
                "media": sorted(t for k, t in slide_rels.values() if k in ("image", "media", "video", "audio")),
                "shapes": shapes(slide_tree, sz),
            }
            (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
            index["slides"].append({"slide": n, "hidden": hidden, "layout": layout_part, "shapes": len(summary["shapes"])})
        (out_root / "index.json").write_text(json.dumps(index, indent=2) + "\n")
    print(f"{deck_id}: extracted XML for {len(index['slides'])} slide(s)")


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("deck_id")
    extract(ap.parse_args().deck_id)
