"""hdgen: generate HD versions of the studio's items in a batch, from the recipes.

    python hdgen.py batch WORK [--classes rooms,scenes,...] [--ids FILE] [--limit N]
                               [--redo] [--recipes DIR] [--plan]

WORK is the studio's work folder (`dune-hd inventory` wrote WORK/review/items.json;
its originals are in true colours). Each chosen item gets a new version in
WORK/versions/<id>/batch-<key>.png, made by its own recipe when it has one
(recipes/heads.json, rooms.json, scenes.json, interface.json) and otherwise by
its class's defaults (recipes/defaults.json) with a hint for its sheet. The
key hashes the picture and every setting: an item whose batch version already
has the same key is skipped, so a batch can be stopped and resumed, and only
what changed in the recipes is redone. --plan only lists what would run.

Methods: picture (rooms, full-screen scenes: 4x, colour-anchored), sprite
(small art: enlarged so the model sees ~1024 px, shrunk to 4x, colour-anchored,
original silhouette), portrait (a head frame padded to a square, like
generate.py), recipe heads (generate.py, all frames of the head).
"""
import argparse
import fnmatch
import hashlib
import json
import os
import subprocess
import sys
import time

import cv2
import numpy as np
from PIL import Image, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
BACK = np.array([20, 14, 10], np.float32)


def load(path, default=None):
    return json.load(open(path)) if os.path.exists(path) else default


# ---------- recipes ----------

class Recipes:
    def __init__(self, d):
        self.dir = d
        self.defaults = load(os.path.join(d, "defaults.json"))
        self.heads = load(os.path.join(d, "heads.json"), {"characters": {}})
        self.rooms = load(os.path.join(d, "rooms.json"), {"rooms": {}, "defaults": {}})
        self.scenes = load(os.path.join(d, "scenes.json"), {"scenes": {}, "defaults": {}})
        self.interface = load(os.path.join(d, "interface.json"), {"items": {}, "defaults": {}})
        models = load(os.path.join(d, "models.json"))
        repo = {m["id"]: m["repo"] for m in models["downloads"]}
        ids = models["classes"]["portrait"]
        self.models = {"model": repo[ids[0]], "controlnet": repo[ids[1]], "vae": repo[ids[2]]}

    def hint(self, group):
        for h in self.defaults["hints"]:
            if any(fnmatch.fnmatch(group or "", g) for g in h["groups"]):
                return h["hint"]
        return "the subject as drawn"

    def eligible(self, it):
        b = self.defaults["batch"].get(it["class_key"], {})
        if not b.get("enabled"):
            return False
        if it["kind"] == "head-frame" and b.get("frames") == "multi-group":
            name = it["group"]
            rec = self.heads["characters"].get(name)
            fid = it["id"].rsplit("/", 1)[1]
            if rec:  # a recipe head: its own frames
                return fid == rec.get("base", "f00") or fid in rec.get("variants", {})
            return len(it.get("groups", [])) >= 2
        mw, mh = b.get("min_size", [0, 0])
        return it["size"][0] >= mw and it["size"][1] >= mh

    def recipe(self, it):
        """(method, settings) for an item: its own recipe, else its class's defaults."""
        cls = self.defaults["classes"][it["class_key"]]
        st = {k: v for k, v in cls.items()}
        st["prompt"] = cls["prompt"].format(hint=self.hint(it.get("group")))
        st["source"] = "class defaults"
        if it["kind"] == "room":
            own = self.rooms["rooms"].get(it.get("art_name"))
            if own:
                st.update({k: v for k, v in self.rooms.get("defaults", {}).items() if k != "part"})
                st.update({k: v for k, v in own.items() if k in ("prompt", "negative", "strength", "cond", "soften", "anchor")})
                st["parts"] = [p if "same_as" not in p else {**own["parts"][p["same_as"]], "box": p["box"]} for p in own.get("parts", [])]
                st["part"] = self.rooms.get("defaults", {}).get("part", {})
                st["source"] = "recipes/rooms.json"
        elif it["class_key"] == "scenes":
            own = self.scenes["scenes"].get(it["group"])
            if own:
                st.update({k: v for k, v in self.scenes.get("defaults", {}).items() if k in ("strength", "cond", "soften", "anchor")})
                st.update({k: v for k, v in own.items() if k in ("prompt", "negative", "strength", "cond", "soften", "anchor")})
                st["source"] = "recipes/scenes.json"
        elif it["class_key"] == "interface":
            own = self.interface["items"].get(it.get("art_name"))
            if own and own.get("prompt"):
                st.update({k: v for k, v in self.interface.get("defaults", {}).items() if k in ("strength", "cond", "soften", "anchor", "negative")})
                st.update({k: v for k, v in own.items() if k in ("prompt", "negative", "strength", "cond", "soften", "anchor")})
                st["source"] = "recipes/interface.json"
            if own and own.get("treat_as") == "scene":
                st["method"] = "picture"
        elif it["kind"] == "head-frame" and it["group"] in self.heads["characters"]:
            return "recipe-head", {"source": "recipes/heads.json", "head": it["group"]}
        return st.pop("method"), st


# ---------- generation ----------

class Pipe:
    def __init__(self, models):
        self.models, self.pipe = models, None

    def __call__(self, guide, st, seed=1992):
        if self.pipe is None:
            import torch
            from diffusers import AutoencoderKL, ControlNetModel, StableDiffusionXLControlNetImg2ImgPipeline
            import device
            print("@stage loading the model", flush=True)
            dev = device.pick()
            dt = dev["dtype"]
            p = StableDiffusionXLControlNetImg2ImgPipeline.from_pretrained(
                self.models["model"], controlnet=ControlNetModel.from_pretrained(self.models["controlnet"], torch_dtype=dt),
                vae=AutoencoderKL.from_pretrained(self.models["vae"], torch_dtype=dt), torch_dtype=dt, variant="fp16")
            self.pipe, self.torch = device.configure(p, dev), torch
        g = self.torch.Generator("cpu").manual_seed(seed)
        return self.pipe(st["prompt"], negative_prompt=st["negative"], image=guide, control_image=guide, strength=st["strength"],
                         width=guide.width, height=guide.height, num_inference_steps=30, controlnet_conditioning_scale=st["cond"],
                         guidance_scale=6.0, generator=g).images[0]


def original(work, it):
    """The original as RGB over a dark backdrop, and its alpha (bool)."""
    im = np.asarray(Image.open(os.path.join(work, it["original"]["path"])).convert("RGBA"), dtype=np.float32)
    alpha = im[..., 3] > 0
    rgb = np.where(alpha[..., None], im[..., :3], BACK)
    return rgb, alpha


def anchor(hd, low_rgb, radius):
    """The HD picture's lightness detail over the original's broad colours (models drift: pink sand → tan)."""
    big = cv2.resize(low_rgb, (hd.shape[1], hd.shape[0]), interpolation=cv2.INTER_CUBIC)
    broad = lambda x: np.stack([cv2.GaussianBlur(x[..., c], (0, 0), radius) for c in range(3)], -1)
    luma = lambda c: c @ np.array([0.2126, 0.7152, 0.0722], np.float32)
    detail = (luma(hd) + 8) / (luma(broad(hd)) + 8)
    return np.clip(broad(big) * detail[..., None], 0, 255)


def guide_of(rgb, w, h, blur):
    return Image.fromarray(rgb.astype(np.uint8)).resize((w, h), Image.BICUBIC).filter(ImageFilter.GaussianBlur(blur))


def picture(pipe, rgb, alpha, st):
    """A whole room or scene, generated at 4x and colour-anchored."""
    h, w = alpha.shape
    W, H = w * 4 // 8 * 8, h * 4 // 8 * 8
    out = np.asarray(pipe(guide_of(rgb, W, H, st["soften"]), st).resize((w * 4, h * 4), Image.LANCZOS), dtype=np.float32)
    for part in st.get("parts", []):
        out = refine(pipe, out, rgb, part, {**st, **st.get("part", {})})
    return anchor(out, rgb, st["anchor"])


def refine(pipe, hd, rgb, part, st):
    """A person or object painted into a room, redone in a 12x close-up and pasted back
    within its outline (cool-coloured pixels in the box, for guards against a warm room)."""
    x0b, y0b, x1b, y1b = part["box"]
    H0, W0 = rgb.shape[:2]
    mask = np.zeros((H0, W0), bool)
    sub = rgb[y0b:y1b, x0b:x1b]
    mask[y0b:y1b, x0b:x1b] = (sub[..., 0] - sub[..., 2]) < 30 if part.get("select") == "cool" else True
    mask = cv2.dilate(mask.astype(np.uint8), np.ones((3, 3), np.uint8)) > 0
    cx, cy = (x0b + x1b) // 2, (y0b + y1b) // 2
    ch = int((y1b - y0b) * 1.4)
    cw = int(ch * 0.8)
    x0, y0 = max(0, cx - cw // 2), max(0, cy - ch // 2)
    x1, y1 = min(W0, x0 + cw), min(H0, y0 + ch)
    z = st.get("zoom", 12)
    gw, gh = (x1 - x0) * z // 8 * 8, (y1 - y0) * z // 8 * 8
    low = guide_of(rgb[y0:y1, x0:x1], gw, gh, st.get("soften", 1.5) * z / 4)
    ctx = Image.fromarray(np.clip(hd, 0, 255).astype(np.uint8)).crop((x0 * 4, y0 * 4, x1 * 4, y1 * 4)).resize((gw, gh), Image.LANCZOS)
    m = Image.fromarray((mask[y0:y1, x0:x1] * 255).astype(np.uint8)).resize((gw, gh), Image.BILINEAR).filter(ImageFilter.GaussianBlur(z * 0.6))
    pst = {**st, "prompt": part["prompt"], "negative": ", ".join(filter(None, [part.get("negative_extra"), st["negative"]]))}
    gen = pipe(Image.composite(low, ctx, m), pst)
    spr = np.asarray(gen.resize(((x1 - x0) * 4, (y1 - y0) * 4), Image.LANCZOS), dtype=np.float32)
    sil = cv2.resize(mask.astype(np.float32), (W0 * 4, H0 * 4), interpolation=cv2.INTER_CUBIC)
    sil = np.clip(cv2.GaussianBlur(sil, (0, 0), 1.5), 0, 1)[y0 * 4:y1 * 4, x0 * 4:x1 * 4, None]
    out = hd.copy()
    out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] = spr * sil + out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] * (1 - sil)
    return out


def sprite(pipe, rgb, alpha, st):
    """Small art: enlarged so the model works near its native size, then 4x, anchored, original outline."""
    h, w = alpha.shape
    z = int(min(16, max(4, round(1024 / max(w, h)))))
    gw, gh = max(64, w * z // 8 * 8), max(64, h * z // 8 * 8)
    out = pipe(guide_of(rgb, gw, gh, st["soften"] * z / 4), st)
    hd = anchor(np.asarray(out.resize((w * 4, h * 4), Image.LANCZOS), dtype=np.float32), rgb, st["anchor"])
    a = cv2.resize(alpha.astype(np.float32), (w * 4, h * 4), interpolation=cv2.INTER_CUBIC)
    a = np.clip((a - 0.5) * 4 + 0.5, 0, 1)
    return np.dstack([hd, a * 255])


def portrait(pipe, rgb, alpha, st):
    """A head frame padded to a square at the bottom (as generate.py does), generated at 1024, cropped back."""
    h, w = alpha.shape
    side = max(w, h)
    sq = np.zeros((side, side, 3), np.float32) + np.array([24, 16, 12], np.float32)
    x0, y0 = (side - w) // 2, side - h
    sq[y0:y0 + h, x0:x0 + w] = rgb
    out = pipe(guide_of(sq, 1024, 1024, st["soften"]), st)
    s = 1024 / side
    return np.asarray(out.crop((round(x0 * s), round(y0 * s), round((x0 + w) * s), round((y0 + h) * s))), dtype=np.float32)


# ---------- batch ----------

def key_of(work, it, method, st, models):
    src = os.path.join(work, it["original"]["path"])
    h = hashlib.sha1(json.dumps({"method": method, "st": st, "models": models, "src": os.path.getsize(src)}, sort_keys=True).encode())
    h.update(open(src, "rb").read())
    return h.hexdigest()[:10]


def save(work, it, pixels, key, method, st):
    d = os.path.join(work, "versions", it["id"].replace("/", "__"))
    os.makedirs(d, exist_ok=True)
    mode = "RGBA" if pixels.shape[-1] == 4 else "RGB"
    Image.fromarray(np.clip(pixels, 0, 255).astype(np.uint8), mode).save(os.path.join(d, f"batch-{key}.png"))
    meta = {"label": f"batch ({st.get('source')}) {time.strftime('%Y-%m-%d')}",
            "settings": {k: v for k, v in st.items() if k not in ("parts", "part")} | {"method": method, "parts": len(st.get("parts", []))}}
    json.dump(meta, open(os.path.join(d, f"batch-{key}.json"), "w"), indent=2)


def batch(a):
    work = os.path.abspath(a.work)
    rec = Recipes(a.recipes)
    items = json.load(open(os.path.join(work, "review", "items.json")))["items"]
    if a.ids:
        wanted = set(json.load(open(a.ids)))
        items = [i for i in items if i["id"] in wanted]
    else:
        classes = set(a.classes.split(",")) if a.classes else None
        items = [i for i in items if (classes is None or i["class_key"] in classes) and rec.eligible(i)]
        # Heads without a recipe: one frame per distinct face (its first group); the
        # others are that face with a gesture or mouth, which needs a recipe to stay consistent.
        faces = set()
        kept = []
        for i in items:
            if i["kind"] == "head-frame" and i["group"] not in rec.heads["characters"]:
                face = (i["group"], i["groups"][0])
                if face in faces:
                    continue
                faces.add(face)
            kept.append(i)
        items = kept
    todo, recipe_heads = [], {}
    for it in items:
        method, st = rec.recipe(it)
        if method == "recipe-head":
            recipe_heads.setdefault(st["head"], []).append(it)
            continue
        key = key_of(work, it, method, st, rec.models)
        if not a.redo and os.path.exists(os.path.join(work, "versions", it["id"].replace("/", "__"), f"batch-{key}.png")):
            continue
        todo.append((it, method, st, key))
    for name in list(recipe_heads):
        done = os.path.join(work, "versions", f"head__{name}__f00")
        if not a.redo and os.path.isdir(done) and any(f.startswith("batch") for f in os.listdir(done)):
            del recipe_heads[name]
    if a.limit:
        todo = todo[:a.limit]
    n = len(todo) + len(recipe_heads)
    if not (a.plan and a.json):
        print(f"@plan {n} items", flush=True)
        print(f"{len(todo)} items by class defaults or recipes, {len(recipe_heads)} recipe heads ({', '.join(recipe_heads) or 'none'}); "
          f"{len(items) - len(todo) - sum(len(v) for v in recipe_heads.values())} already done", flush=True)
    if a.plan and a.json:
        by = {}
        for it, method, st, key in todo:
            by[it["class_key"]] = by.get(it["class_key"], 0) + 1
        for name, its in recipe_heads.items():
            by["heads"] = by.get("heads", 0) + 1
        print(json.dumps({"total": n, "by_class": by, "recipe_heads": list(recipe_heads), "considered": len(items)}))
        return
    if a.plan:
        for it, method, st, key in todo:
            print(f"  {it['id']:32} {method:9} {st['source']:22} {st['prompt'][:90]}")
        return
    k = 0
    # Recipe heads first, in their own process (generate.py), before this one loads the model:
    # two copies would not fit a 16 GB machine.
    for name in recipe_heads:
        k += 1
        print(f"@stage {name} (recipe) ({k}/{n})", flush=True)
        heads_dir = os.path.join(work, "heads")
        if not os.path.exists(os.path.join(heads_dir, f"{name}-f00.png")):
            if not (a.dune_hd and a.dat):
                print(f"!! {name}: no heads dump in {heads_dir} (give --dune-hd and --dat)", flush=True)
                continue
            subprocess.run([a.dune_hd, "heads", "--dat", a.dat, "--work", work], check=False)
        r = subprocess.run([sys.executable, os.path.join(HERE, "generate.py"), name, "--heads", heads_dir, "--characters",
                            os.path.join(a.recipes, "heads.json"), "--out", os.path.join(work, "gen", name), "--publish", work,
                            "--label", f"batch (recipes/heads.json) {time.strftime('%Y-%m-%d')}"])
        if r.returncode:
            print(f"!! {name}: generate.py failed", flush=True)
    pipe = Pipe(rec.models)
    failed = 0
    for it, method, st, key in todo:
        k += 1
        print(f"@stage {it['title']} ({k}/{n})", flush=True)
        t = time.time()
        try:
            rgb, alpha = original(work, it)
            px = {"picture": picture, "sprite": sprite, "portrait": portrait}[method](pipe, rgb, alpha, st)
            save(work, it, px, key, method, st)
            print(f"{it['id']}: {time.time() - t:.0f}s", flush=True)
        except Exception as e:  # noqa: BLE001 — one item's failure does not stop the batch
            failed += 1
            print(f"!! {it['id']}: {type(e).__name__}: {e}", flush=True)
    print(f"@progress 1.0", flush=True)
    print(f"batch done: {len(todo) - failed} made, {failed} failed", flush=True)
    return 1 if failed and failed == len(todo) else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("batch")
    b.add_argument("work")
    b.add_argument("--classes", help="class keys (rooms, scenes, heads, characters, interface, other, room-pieces)")
    b.add_argument("--ids", help="JSON file with a list of item ids (overrides --classes and the batch rules)")
    b.add_argument("--limit", type=int)
    b.add_argument("--redo", action="store_true", help="generate even items whose batch version is up to date")
    b.add_argument("--plan", action="store_true", help="only list what would be generated")
    b.add_argument("--json", action="store_true", help="with --plan: a JSON summary")
    b.add_argument("--recipes", default=os.path.join(HERE, "..", "..", "recipes"))
    b.add_argument("--dune-hd", help="dune-hd, to extract the talking heads if WORK/heads is missing")
    b.add_argument("--dat", help="DUNE.DAT, with --dune-hd")
    a = ap.parse_args()
    a.recipes = os.path.abspath(a.recipes)
    sys.exit(batch(a) or 0)


if __name__ == "__main__":
    main()
