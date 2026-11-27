"""A pack built by `dune-hd build` (any backend, e.g. an Upscayl model) brought into the
studio as a pack of its own, to look at it there: each item gets one version, the art
as the pack shows it in the game.

    python import_pack.py WORK PACK.zip [--build-work DIR] [--title T] [--pack ID] [--label L]

The colours come from the build's previews (its --work folder, WORK by default the
zip's sibling `work`, built with --previews: the third panel, the pack's texels in the
art's palette), the coverage from the pack's texels. Talking-head poses are put
together from their parts as the game draws them; a part the pack left to the built-in
upscaler is shown as pixel art, and the version's label says how many. Items the pack
has no art for get no version.

Makes a new pack (named after the pack's model, or --title) and makes it active, or adds
to --pack (its new versions become the defaults; --label tells them from the earlier
ones). An imported pack has no recipes of its own.
"""
import argparse
import json
import os
import sys
import time
import zipfile
from io import BytesIO

import numpy as np
from PIL import Image

import packs

GREY = (40, 40, 40)  # the previews' background


class Build:
    def __init__(self, zip_path, work):
        self.zip = zipfile.ZipFile(zip_path)
        self.names = set(self.zip.namelist())
        self.manifest = json.loads(self.zip.read("manifest.json"))
        self.preview = os.path.join(work, "preview")
        self.by_part = {(a["res"].removesuffix(".HSQ"), a["part"]): a for a in self.manifest["assets"]}
        self.by_id = {a["id"]: a for a in self.manifest["assets"]}

    def panels(self, name):
        p = os.path.join(self.preview, name + ".png")
        if not os.path.exists(p):
            return None
        im = Image.open(p).convert("RGB")
        w = im.width // 3
        return [im.crop((i * w, 0, (i + 1) * w, im.height)) for i in range(3)]

    def coverage(self, folder, aid):
        n = f"{folder}/{aid}.png"
        return Image.open(BytesIO(self.zip.read(n))).convert("RGBA").getchannel("A") if n in self.names else None

    def art(self, name, folder, aid, width=None):
        """The pack's art at 4x, RGBA, or None."""
        pan = self.panels(name)
        a = self.coverage(folder, aid) if aid and pan else None
        if a is None:
            return None
        im = pan[2].convert("RGBA")
        im.putalpha(a.resize(im.size))
        return im.crop((0, 0, width * 4, im.height)) if width else im

    def solid(self, name):
        """Where a sprite draws, at 4x (from its preview's first panel), or None."""
        pan = self.panels(name)
        if not pan:
            return None
        return Image.fromarray(np.where((np.asarray(pan[0]) == GREY).all(axis=2), 0, 255).astype(np.uint8), "L")


def head_pose(build, name, layout, groups, original):
    """A talking-head pose at 4x, its parts drawn in order as the game draws them, the
    original pixel art for the parts the pack leaves to the built-in upscaler: (image,
    parts left out, parts in the pack)."""
    parts = [p for g in groups for p in layout["groups"][g]]
    orig = original.convert("RGBA").resize((original.width * 4, original.height * 4), Image.NEAREST)
    x0, y0 = min(x for _, x, _ in parts), min(y for _, _, y in parts)  # as heads::compose
    out = Image.new("RGBA", orig.size)
    left, used = 0, 0
    for sprite, x, y in parts:
        a = build.by_part.get((name, sprite))
        at = ((x - x0) * 4, (y - y0) * 4)
        im = build.art(f"{name}-{sprite:03d}", "a4", a["id"]) if a else None
        if im is not None:
            out.alpha_composite(im, at)
            used += 1
            continue
        left += 1
        mask = build.solid(f"{name}-{sprite:03d}")
        if mask is not None:
            box = (at[0], at[1], at[0] + mask.width, at[1] + mask.height)
            tile = orig.crop(box)
            tile.putalpha(Image.fromarray(np.minimum(np.asarray(tile.getchannel("A")), np.asarray(mask)), "L"))
            out.alpha_composite(tile, at)
    # Anything still empty that the original draws (parts without a preview): the original.
    o, n = np.asarray(orig), np.asarray(out).copy()
    hole = (n[..., 3] == 0) & (o[..., 3] > 0)
    n[hole] = o[hole]
    return Image.fromarray(n, "RGBA"), left, used


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("work")
    ap.add_argument("zip")
    ap.add_argument("--build-work", help="the build's --work folder, with preview/ (default: next to the zip)")
    ap.add_argument("--title")
    ap.add_argument("--pack", help="an existing pack to add the versions to")
    ap.add_argument("--label", help="the versions' label (default: MODEL (imported pack))")
    a = ap.parse_args()
    build = Build(a.zip, a.build_work or os.path.join(os.path.dirname(os.path.abspath(a.zip)), "work"))
    if not os.path.isdir(build.preview):
        sys.exit(f"no previews in {build.preview}: build with --previews, or pass --build-work")
    model = build.manifest.get("model", {}).get("name") or os.path.basename(a.zip)
    if a.pack:
        pid = packs.get(a.work, a.pack)["id"]
    else:
        title = a.title or model
        cfg = packs.add(a.work, title, imported={"zip": os.path.abspath(a.zip), "model": model})
        pid = cfg["active"]
    items = json.load(open(os.path.join(a.work, "review", "items.json"), encoding="utf-8"))["items"]
    heads = {}
    stamp = time.strftime("%Y%m%d-%H%M%S")
    label = a.label or f"{model} (imported pack)"
    done, partial, none = 0, 0, []
    for it in items:
        kind = it["kind"]
        if kind == "sprite":
            aid = it.get("content")
            asset = build.by_id.get(aid)
            im = build.art(f"{asset['res'].removesuffix('.HSQ')}-{asset['part']:03d}", "a4", aid, it["size"][0]) if asset else None
            missing = 0
        elif kind == "room":
            rid = it["id"].split("/", 1)[1]
            room = next((r for r in build.manifest["rooms"] if r["id"] == rid), None)
            im = build.art(f"ROOM-{room['file']}-{room['room']:03d}", "r4", rid) if room else None
            missing = 0
        else:
            name = it["group"]
            if name not in heads:
                heads[name] = json.load(open(os.path.join(a.work, "heads", name + ".json")))
            original = Image.open(os.path.join(a.work, it["original"]["path"]))
            im, missing, used = head_pose(build, name, heads[name], it["groups"], original)
            if not used:
                im = None  # none of its parts in the pack
        if im is None:
            none.append(it["id"])
            continue
        d = os.path.join(a.work, "versions", it["id"].replace("/", "__"))
        os.makedirs(d, exist_ok=True)
        vid = f"import-{pid}-{stamp}"
        im.save(os.path.join(d, vid + ".png"))
        meta = {"label": label + (f", {missing} parts pixel art (left to the built-in upscaler)" if missing else ""),
                "settings": {"model": model, "source": os.path.abspath(a.zip), "method": "imported pack"},
                "pack": pid, "default": True}
        json.dump(meta, open(os.path.join(d, vid + ".json"), "w"), indent=2)
        done += 1
        partial += bool(missing)
    print(f"pack {pid}: {done} items imported ({partial} talking-head poses with parts left to the built-in upscaler), "
          f"{len(none)} without art in the pack")


if __name__ == "__main__":
    main()
