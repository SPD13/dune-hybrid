"""Export the studio's chosen versions as finished art for `dune-hd build --art`.

    python export_pack.py WORK --out DIR [--scope approved|all]

For each item it takes one version: the preferred one, else the item's
default (the newest made by its recipe). With --scope approved (the default)
only items whose status is approved; with --scope all every item with a
version, except rejected ones (to see the work in progress in the game).

Each picture is written as DIR/<art_name>.png, the name `dune-hd build --art`
expects: talking heads NAME-fNN / NAME-vGG at their own size (dune-hd resizes
them to 4×), everything else at exactly 4× the original. Sidecars
(DIR/<art_name>.json):
- "dac": the palette the original was shown in, when it is not the sprite's
  own sheet's (WORK/inventory/palettes, from `dune-hd inventory`), in 6-bit
  VGA values, so the art is projected onto the right colours;
- "true_colour": for the classes whose art keeps its own colours in the game
  (TRUE_COLOUR: characters drawn over rooms). Talking heads are always in true
  colour.
DIR/export.json lists what was exported and what was left out, and why.
"""
import argparse
import json
import os
import shutil
import sys

# Classes stored in true colour (pack version 4); the others are projected onto
# the game's palette, which keeps their palette effects (day and night, fades).
TRUE_COLOUR = {"characters"}


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    return int.from_bytes(head[16:20], "big"), int.from_bytes(head[20:24], "big")


def versions(work, item_id):
    """The item's versions: {id: path}, oldest first (as the studio lists them)."""
    d = os.path.join(work, "versions", item_id.replace("/", "__"))
    if not os.path.isdir(d):
        return {}
    pngs = sorted((n for n in os.listdir(d) if n.endswith(".png")), key=lambda n: (os.path.getmtime(os.path.join(d, n)), n))
    return {n[:-4]: os.path.join(d, n) for n in pngs}


def default_version(work, item_id, found):
    """The version marked default by its sidecar (the newest such), else the newest."""
    marked = [v for v, p in found.items() if os.path.exists(p[:-4] + ".json") and json.load(open(p[:-4] + ".json")).get("default")]
    return (marked or list(found))[-1]


def art_name(it):
    if it.get("art_name"):
        return it["art_name"]
    kind, _, rest = it["id"].partition("/")
    return rest.replace("/", "-") if kind == "head" else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("work")
    ap.add_argument("--out", required=True)
    ap.add_argument("--scope", choices=["approved", "all"], default="approved")
    a = ap.parse_args()
    work = os.path.abspath(a.work)
    items = json.load(open(os.path.join(work, "review", "items.json")))["items"]
    review_path = os.path.join(work, "review", "review.json")
    review = json.load(open(review_path, encoding="utf-8"))["assets"] if os.path.exists(review_path) else {}
    if os.path.isdir(a.out):
        shutil.rmtree(a.out)
    os.makedirs(a.out)
    done, skipped = [], []
    for it in items:
        r = review.get(it["id"], {})
        status = r.get("status", "new")
        if status == "rejected" or (a.scope == "approved" and status != "approved"):
            continue
        found = versions(work, it["id"])
        if not found:
            if status == "approved":
                skipped.append({"id": it["id"], "why": "approved, but no generated version"})
            continue
        vid = r.get("preferred") if r.get("preferred") in found else default_version(work, it["id"], found)
        name = art_name(it)
        if not name:
            skipped.append({"id": it["id"], "why": "no art name (run dune-hd inventory again)"})
            continue
        src = found[vid]
        dst = os.path.join(a.out, name + ".png")
        w, h = png_size(src)
        head = it["kind"] in ("head-frame", "head-group")
        tw, th = it["size"][0] * 4, it["size"][1] * 4
        if head or (w, h) == (tw, th):
            shutil.copyfile(src, dst)
        else:
            if abs(w / h - tw / th) > 0.02 * tw / th:
                skipped.append({"id": it["id"], "why": f"version {vid} is {w}×{h}, not at the original's aspect ({tw}×{th} expected)"})
                continue
            from PIL import Image  # only when a picture needs resizing (the generation environment has it)
            Image.open(src).convert("RGB").resize((tw, th), Image.LANCZOS).save(dst)
        side = {}
        dac = os.path.join(work, "inventory", "palettes", name + ".dac")
        if not head and os.path.exists(dac):
            side["dac"] = bytes(v >> 2 for v in open(dac, "rb").read()).hex()
        if it.get("class_key") in TRUE_COLOUR:
            side["true_colour"] = True
        if side:
            json.dump(side, open(os.path.join(a.out, name + ".json"), "w"))
        done.append({"id": it["id"], "art_name": name, "version": vid, "status": status, **({"true_colour": True} if head or side.get("true_colour") else {})})
    json.dump({"scope": a.scope, "exported": done, "skipped": skipped}, open(os.path.join(a.out, "export.json"), "w"), indent=2)
    heads = sum(1 for d in done if d["id"].startswith("head/"))
    print(f"exported {len(done)} pictures ({heads} talking-head frames and groups) to {a.out}", flush=True)
    for s in skipped:
        print(f"!! {s['id']}: {s['why']}", flush=True)
    print("@progress 1.0", flush=True)
    return 0 if done else 1


if __name__ == "__main__":
    sys.exit(main())
