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
# Where emblems (vector masters, NAME.svg) are looked up, first found wins: the work
# folder (local masters), then the recipes. Set by batch().
EMBLEM_DIRS = []
DUNE_HD = None


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
        self.cast = load(os.path.join(d, "cast.json"), {"characters": {}})["characters"]
        self.style = load(os.path.join(d, "style.json"), {})
        models = load(os.path.join(d, "models.json"))
        repo = {m["id"]: m["repo"] for m in models["downloads"]}
        self.downloads = {m["id"]: m for m in models["downloads"]}
        self.model_rules = models.get("use", {}).get("rules", [])
        ids = models["classes"]["portrait"]
        self.models = {"model": repo[ids[0]], "controlnet": repo[ids[1]], "vae": repo[ids[2]]}

    def part(self, p, parts):
        """A room part with what it refers to filled in: same_as (another part) and character
        (recipes/cast.json); the part's own keys win. A character's prompt is its look followed
        by its pose, so a part can give its own pose and keep the same person."""
        if "same_as" in p:
            p = {**self.part(parts[p["same_as"]], parts), **{k: v for k, v in p.items() if k != "same_as"}}
        if "character" in p:
            c = self.cast[p["character"]]
            p = {**c, **p}
            if "prompt" not in p or p["prompt"] == c.get("prompt"):
                p["prompt"] = ", ".join(filter(None, [c.get("prompt"), p.get("pose")]))
            p.pop("pose", None)
        return p

    def hint(self, group):
        for h in self.defaults["hints"]:
            if any(fnmatch.fnmatch(group or "", g) for g in h["groups"]):
                return h["hint"]
        return "the subject as drawn"

    def eligible(self, it):
        b = self.defaults["batch"].get(it["class_key"], {})
        if not b.get("enabled"):
            return False
        if it["kind"] == "head-group":  # a talking group: made from its head's recipe (generate.py)
            return it["group"] in self.heads["characters"]
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
            # Every room takes the rooms' defaults (settings, detail pass); one with its own
            # recipe adds its prompt, parts and settings on top.
            st.update({k: v for k, v in self.rooms.get("defaults", {}).items() if k != "part"})
            own = self.rooms["rooms"].get(it.get("art_name"))
            if own:
                st.update({k: v for k, v in own.items() if k in ("prompt", "negative", "strength", "cond", "soften", "anchor", "detail", "style", "lights", "model", "keep_in_guide")})
                st["parts"] = [self.part(p, own["parts"]) for p in own.get("parts", [])]
                st["part"] = self.rooms.get("defaults", {}).get("part", {})
                st["source"] = "recipes/rooms.json"
        elif it["class_key"] == "scenes":
            own = self.scenes["scenes"].get(it["group"])
            if own:
                st.update({k: v for k, v in self.scenes.get("defaults", {}).items() if k in ("strength", "cond", "soften", "anchor")})
                st.update({k: v for k, v in own.items() if k in ("prompt", "negative", "strength", "cond", "soften", "anchor", "style")})
                st["source"] = "recipes/scenes.json"
        elif it["class_key"] == "interface":
            own = self.interface["items"].get(it.get("art_name"))
            if own and own.get("prompt"):
                st.update({k: v for k, v in self.interface.get("defaults", {}).items() if k in ("strength", "cond", "soften", "anchor", "negative")})
                st.update({k: v for k, v in own.items() if k in ("prompt", "negative", "strength", "cond", "soften", "anchor", "style")})
                st["source"] = "recipes/interface.json"
            if own and own.get("treat_as") == "scene":
                st["method"] = "picture"
        elif it["kind"] in ("head-frame", "head-group") and it["group"] in self.heads["characters"]:
            return "recipe-head", {"source": "recipes/heads.json", "head": it["group"]}
        if st.get("style", True) is not False:
            st["style"] = self.style_for(it)
        if st.get("model") in self.downloads:  # a recipe's own model (an id from models.json) wins
            st["model"] = self.downloads[st["model"]]["repo"]
        for r in [] if st.get("model") else self.model_rules:  # models.json "use": the base model for this item
            if ("classes" not in r or it["class_key"] in r["classes"]) and                ("groups" not in r or any(fnmatch.fnmatch(it.get("group") or "", g) for g in r["groups"])):
                st["model"] = self.downloads[r["model"]]["repo"]
                break
        return st.pop("method"), st

    def style_for(self, it):
        """The style layers above an item's own prompt (recipes/style.json): the game's,
        then its location's, matched on the item's group."""
        game = self.style.get("game")
        if not game:
            return None
        layers, negs, where = [game["prompt"]], [game.get("negative", "")], None
        for name, loc in self.style.get("locations", {}).items():
            if any(fnmatch.fnmatch(it.get("group") or "", g) for g in loc.get("groups", [])):
                layers.append(loc["prompt"])
                negs.append(loc.get("negative", ""))
                where = name
                break
        return {"layers": layers, "negative": ", ".join(filter(None, negs)), "location": where}


# ---------- generation ----------

class Pipe:
    def __init__(self, models):
        self.models, self.pipe, self.loaded = models, None, None

    def __call__(self, guide, st, seed=1992):
        want = st.get("model") or self.models["model"]  # a try may pick another base model
        if self.pipe is not None and self.loaded != want:
            self.pipe = None
            self.torch.cuda.empty_cache() if self.torch.cuda.is_available() else None
        if self.pipe is None:
            import torch
            from diffusers import AutoencoderKL, ControlNetModel, StableDiffusionXLControlNetImg2ImgPipeline
            import device
            print("@stage loading the model", flush=True)
            dev = device.pick()
            dt = dev["dtype"]
            p = StableDiffusionXLControlNetImg2ImgPipeline.from_pretrained(
                want, controlnet=ControlNetModel.from_pretrained(self.models["controlnet"], torch_dtype=dt),
                vae=AutoencoderKL.from_pretrained(self.models["vae"], torch_dtype=dt), torch_dtype=dt, variant="fp16")
            self.pipe, self.torch, self.loaded = device.configure(p, dev), torch, want
        g = self.torch.Generator("cpu").manual_seed(seed)
        args = dict(image=guide, control_image=guide, strength=st["strength"], width=guide.width, height=guide.height,
                    num_inference_steps=30, controlnet_conditioning_scale=st["cond"], guidance_scale=6.0, generator=g)
        style = st.get("style") if isinstance(st.get("style"), dict) else None
        if not style:
            return self.pipe(st["prompt"], negative_prompt=st["negative"], **args).images[0]
        negative = ", ".join(filter(None, [st["negative"], style.get("negative")]))
        return self.pipe(**self.embed(style["layers"] + [st["prompt"]], negative), **args).images[0]

    def chunks(self, text):
        """A prompt as pieces of up to 77 CLIP tokens, cut between comma-separated phrases."""
        out, cur = [], []
        for phrase in [p.strip() for p in text.split(",") if p.strip()]:
            if cur and len(self.pipe.tokenizer(", ".join(cur + [phrase])).input_ids) > 77:
                out.append(", ".join(cur))
                cur = []
            cur.append(phrase)
        return out + ([", ".join(cur)] if cur else [])

    def embed(self, layers, negative):
        """Long prompts: each layer (game style, location, item) and each 77-token piece of it
        encoded on its own, the embeddings joined. The pooled embedding (the overall gist)
        is the item's own."""
        pos = [c for layer in layers for c in self.chunks(layer)]
        item = len(pos) - len(self.chunks(layers[-1]))
        neg = self.chunks(negative) or [""]
        n = max(len(pos), len(neg))
        pos, neg = pos + [""] * (n - len(pos)), neg + [""] * (n - len(neg))
        dev = self.pipe._execution_device
        enc = [self.pipe.encode_prompt(p, device=dev, do_classifier_free_guidance=True, negative_prompt=q) for p, q in zip(pos, neg)]
        cat = lambda i: self.torch.cat([e[i] for e in enc], dim=1)
        return {"prompt_embeds": cat(0), "negative_prompt_embeds": cat(1),
                "pooled_prompt_embeds": enc[item][2], "negative_pooled_prompt_embeds": enc[0][3]}


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
    parts = st.get("parts", [])
    # Emblems are taken out of the pixel art before anything is generated (the model would
    # paint its own version, larger than the original's pixels), and drawn back at the end.
    src = without_emblems(rgb, [p for p in parts if "emblem" in p])
    # Banners are taken out the same way, so the model paints what is behind them and the
    # anchoring does not spread their green around; they are drawn back at the very end.
    banners = [p for p in parts if "banner" in p]
    for p in banners:
        src = inpaint_mask(src, banner_mask(rgb, p))
    # Erased parts (erase: true; their pixels: select in the box) are painted out of the pixel
    # art: the room is generated without them.
    for p in parts:
        if p.get("erase"):
            src = inpaint_mask(src, part_mask(rgb, p))
    # keep_in_guide: the room is generated with its banners still in the pixel art, when the
    # room's look came out better that way; they are still taken out of the colours the
    # result is anchored to, and drawn over at the end. Emblems always go before generation.
    seen = without_emblems(rgb, [p for p in parts if "emblem" in p]) if st.get("keep_in_guide") else src
    out = np.asarray(pipe(guide_of(seen, W, H, st["soften"]), st, st.get("seed", 1992)).resize((w * 4, h * 4), Image.LANCZOS), dtype=np.float32)
    ref = src.copy()  # the colours the result is anchored to (emblems and banners taken out)
    # Order: people and objects in close-up, then the detail pass over everything (so the
    # room is as crisp as the close-ups, and these keep the look they had), the anchoring,
    # then emblems and banners, which must keep their exact shape, and lights.
    keep = np.zeros((h * 4, w * 4, 1), np.float32)  # sprites drawn so far: harmonizing leaves them
    for part in parts:
        if "select" in part and "emblem" not in part and "banner" not in part and not part.get("erase"):
            keep = np.maximum(keep, silhouette(part_mask(src, part), part.get("feather", 1.5) + 2))
        if "copy_of" in part:
            out = copy_part(out, src, parts[part["copy_of"]], part)
        elif "emblem" not in part and "banner" not in part and not part.get("erase"):
            own = {k: part[k] for k in ("strength", "cond", "soften", "zoom") if k in part}
            out = refine(pipe, out, src, part, {**st, **st.get("part", {}), **own})
            h = part.get("harmonize", st.get("part", {}).get("harmonize"))
            if is_area(part) and h:
                out = harmonize(pipe, out, part, st, h, keep)
    if st.get("detail"):
        out = detail(pipe, out, st)
    out = anchor(out, ref, st["anchor"])
    # Emblems last, over the anchored picture (whose colours were taken without them): the
    # anchoring would soften their edges against a dark wall.
    for part in parts:
        if "emblem" in part:
            out = emblem_part(pipe, out, rgb, part, st)
    for p in banners:
        out = banner_part(out, rgb, p)
    return lights(out, rgb, st["lights"]) if st.get("lights") else out


def banner_box(part, width):
    """A banner part's corners (TL, TR, BR, BL, low-res pixel-edge coordinates; flip: the
    mirrored banner on the other side) and the box around them."""
    q = np.array(part["quad"], np.float32)
    if part.get("flip"):
        q = q[[1, 0, 3, 2]] * [-1, 1] + [width, 0]
    x0, y0 = np.floor(q.min(0)).astype(int) - 2
    x1, y1 = np.ceil(q.max(0)).astype(int) + 2
    return q, [max(0, x0), max(0, y0), x1, y1]


def banner_mask(rgb, part):
    """A banner's pixels in the original: its green pixels and the dark ones drawn inside
    them (the black strips), within its box."""
    x0, y0, x1, y1 = banner_box(part, rgb.shape[1])[1]
    x1, y1 = min(x1, rgb.shape[1]), min(y1, rgb.shape[0])
    sub = rgb[y0:y1, x0:x1]
    sel = ((sub[..., 1] - np.maximum(sub[..., 0], sub[..., 2])) > 30).astype(np.uint8)
    near = cv2.dilate(sel, np.ones((3, 3), np.uint8), iterations=2) > 0
    m = np.zeros(rgb.shape[:2], bool)
    m[y0:y1, x0:x1] = (sel > 0) | (near & (sub.sum(-1) < 60))
    return m


def inpaint_mask(rgb, mask):
    m = cv2.dilate(mask.astype(np.uint8), np.ones((3, 3), np.uint8))
    return cv2.inpaint(np.clip(rgb, 0, 255).astype(np.uint8), m, 3, cv2.INPAINT_TELEA).astype(np.float32)


def banner_part(hd, rgb, part):
    """A banner drawn from its vector master (banner: NAME, like emblems), not generated:
    the master is warped onto the banner's four corners (quad), so every banner has the
    same design with clean edges and pointed tips, in the room's perspective."""
    q, (x0, y0, x1, y1) = banner_box(part, rgb.shape[1])
    H, W = hd.shape[:2]
    x1, y1 = min(x1, W // 4), min(y1, H // 4)
    tw, th = 200, 1160
    em = render_emblem(part["banner"], tw, th)
    src = np.float32([[0, 0], [tw, 0], [tw, th], [0, th]])
    k = 4  # drawn 4x larger, then shrunk: smooth edges
    dst = (q - [x0, y0]) * 4 * k
    M = cv2.getPerspectiveTransform(src, dst.astype(np.float32))
    w, h = (x1 - x0) * 4, (y1 - y0) * 4
    big = cv2.warpPerspective(em, M, (w * k, h * k), flags=cv2.INTER_LINEAR)
    big[..., :3] *= big[..., 3:] / 255  # premultiplied, so the shrink does not darken the edges
    warped = cv2.resize(big, (w, h), interpolation=cv2.INTER_AREA)
    warped[..., :3] /= np.maximum(warped[..., 3:] / 255, 1e-6)
    a = warped[..., 3:] / 255
    out = hd.copy()
    region = out[y0 * 4:y1 * 4, x0 * 4:x1 * 4]
    region[:] = warped[..., :3] * a + region * (1 - a)
    return out


def emblem_pixels(rgb, box):
    """The emblem's own pixels in its box: pure bright reds (not a dark red wall behind it)."""
    x0, y0, x1, y1 = box
    sub = rgb[y0:y1, x0:x1]
    return (sub[..., 0] > 90) & (np.maximum(sub[..., 1], sub[..., 2]) < sub[..., 0] * 0.25)


def without_emblems(rgb, emblems):
    """The pixel art with each emblem's pixels filled from the wall around them."""
    if not emblems:
        return rgb
    out = rgb.copy()
    for e in emblems:
        x0, y0, x1, y1 = e["box"]
        mask = np.zeros(rgb.shape[:2], np.uint8)
        mask[y0:y1, x0:x1] = emblem_pixels(rgb, e["box"])
        mask = cv2.dilate(mask, np.ones((3, 3), np.uint8))
        out = cv2.inpaint(np.clip(out, 0, 255).astype(np.uint8), mask, 3, cv2.INPAINT_TELEA).astype(np.float32)
    return out


def detail(pipe, hd, st):
    """A detail pass over the whole picture, so it is as crisp as the parts redone in
    close-up: enlarged by scale, repainted in overlapping tiles at low strength (the
    tile ControlNet keeps the layout), blended with feathered weights, shrunk back."""
    d = st["detail"]
    k, T, ov = d.get("scale", 2), d.get("tile", 1024), d.get("overlap", 256)
    H, W = hd.shape[:2]
    big = np.asarray(Image.fromarray(np.clip(hd, 0, 255).astype(np.uint8)).resize((W * k, H * k), Image.LANCZOS), dtype=np.float32)
    BH, BW = big.shape[:2]
    starts = lambda n: sorted({min(i, max(0, n - T)) for i in range(0, max(1, n - ov), T - ov)})
    acc, wsum = np.zeros_like(big), np.zeros(big.shape[:2] + (1,), np.float32)
    pst = {**st, "strength": d.get("strength", 0.4), "cond": d.get("cond", 0.6)}
    for ty in starts(BH):
        for tx in starts(BW):
            th, tw = min(T, BH - ty) // 8 * 8, min(T, BW - tx) // 8 * 8
            tile = Image.fromarray(big[ty:ty + th, tx:tx + tw].astype(np.uint8))
            gen = np.asarray(pipe(tile, pst, d.get("seed", 1992)), dtype=np.float32)
            # Weights fall off towards the tile's inner edges: neighbours blend without seams.
            ramp = lambda n: np.minimum(np.minimum(np.arange(n) + 1, n - np.arange(n)) / ov, 1)
            wy, wx = ramp(th), ramp(tw)
            if ty == 0: wy[:th // 2] = 1
            if ty + th >= BH - 7: wy[th // 2:] = 1
            if tx == 0: wx[:tw // 2] = 1
            if tx + tw >= BW - 7: wx[tw // 2:] = 1
            wgt = (wy[:, None] * wx[None, :])[..., None]
            acc[ty:ty + th, tx:tx + tw] += gen * wgt
            wsum[ty:ty + th, tx:tx + tw] += wgt
    big = np.where(wsum > 0, acc / np.maximum(wsum, 1e-6), big)
    return np.asarray(Image.fromarray(np.clip(big, 0, 255).astype(np.uint8)).resize((W, H), Image.LANCZOS), dtype=np.float32)


def emblem_file(name):
    for d in EMBLEM_DIRS:
        f = os.path.join(d, name + ".svg")
        if os.path.exists(f):
            return f
    raise FileNotFoundError(f"emblem {name}: no {name}.svg in {EMBLEM_DIRS}")


def dune_hd():
    if DUNE_HD:
        return DUNE_HD
    for name in ("dune-hd.exe", "dune-hd"):
        f = os.path.join(HERE, "..", "..", "target", "release", name)
        if os.path.exists(f):
            return f
    raise FileNotFoundError("dune-hd not found (give --dune-hd)")


def render_emblem(name, w, h):
    """The emblem's vector master as RGBA float pixels, w x h."""
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        out = os.path.join(d, "e.png")
        subprocess.run([dune_hd(), "svg", emblem_file(name), out, "--width", str(w)], check=True)
        im = Image.open(out).convert("RGBA").resize((w, h), Image.LANCZOS)
    return np.asarray(im, dtype=np.float32)


def emblem_part(pipe, hd, rgb, part, st, ref=None):
    """An emblem drawn from its vector master, the same everywhere: the room's own
    version (red pixels in the box) is painted out, the master pasted in the box in the
    original's colour, then a light pass gives it the surface's material. ref (the colours
    the picture is anchored to) gets the same change at low resolution, so the anchoring
    follows the new shape instead of dimming it and tinting the wall around."""
    x0, y0, x1, y1 = part["box"]
    W, H = (x1 - x0) * 4, (y1 - y0) * 4
    em = render_emblem(part["emblem"], W, H)
    if part.get("flip"):
        em = em[:, ::-1]
    a = em[..., 3:] / 255
    sub = rgb[y0:y1, x0:x1]
    red = emblem_pixels(rgb, part["box"])
    colour = np.array(part["colour"], np.float32) if "colour" in part else (
        np.percentile(sub[red], 75, axis=0) if red.any() else em[..., :3][a[..., 0] > 0.5].mean(0))
    out = hd.copy()
    crop = np.clip(out[y0 * 4:y1 * 4, x0 * 4:x1 * 4], 0, 255).astype(np.uint8)
    old = cv2.dilate(cv2.resize(red.astype(np.uint8), (W, H), interpolation=cv2.INTER_NEAREST), np.ones((9, 9), np.uint8))
    crop = cv2.inpaint(crop, old, 6, cv2.INPAINT_TELEA).astype(np.float32)
    out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] = crop * (1 - a) + colour * a
    if ref is not None:
        low = np.clip(ref[y0:y1, x0:x1], 0, 255).astype(np.uint8)
        low = cv2.inpaint(low, cv2.dilate(red.astype(np.uint8), np.ones((3, 3), np.uint8)), 2, cv2.INPAINT_TELEA).astype(np.float32)
        la = cv2.resize(a[..., 0], (x1 - x0, y1 - y0), interpolation=cv2.INTER_AREA)[..., None]
        ref[y0:y1, x0:x1] = low * (1 - la) + colour * la
    if not part.get("material"):
        return out
    m = int(part.get("margin", 3))
    X0, Y0, X1, Y1 = max(0, x0 - m), max(0, y0 - m), min(rgb.shape[1], x1 + m), min(rgb.shape[0], y1 + m)
    z = max(1.0, 1024 / (max(X1 - X0, Y1 - Y0) * 4))
    gw, gh = int((X1 - X0) * 4 * z) // 8 * 8, int((Y1 - Y0) * 4 * z) // 8 * 8
    region = Image.fromarray(np.clip(out[Y0 * 4:Y1 * 4, X0 * 4:X1 * 4], 0, 255).astype(np.uint8)).resize((gw, gh), Image.LANCZOS)
    pst = {**st, "prompt": part["material"], "strength": part.get("strength", 0.35), "cond": part.get("cond", 0.8),
           "negative": ", ".join(filter(None, [part.get("negative_extra"), st["negative"]]))}
    gen = np.asarray(pipe(region, pst, part.get("seed", 1992)).resize(((X1 - X0) * 4, (Y1 - Y0) * 4), Image.LANCZOS), dtype=np.float32)
    keep = np.zeros(((Y1 - Y0) * 4, (X1 - X0) * 4), np.float32)
    keep[(y0 - Y0) * 4:(y1 - Y0) * 4, (x0 - X0) * 4:(x1 - X0) * 4] = np.maximum(a[..., 0], old > 0)
    keep = np.clip(cv2.GaussianBlur(cv2.dilate(keep, np.ones((7, 7), np.uint8)), (0, 0), 3), 0, 1)[..., None]
    out[Y0 * 4:Y1 * 4, X0 * 4:X1 * 4] = gen * keep + out[Y0 * 4:Y1 * 4, X0 * 4:X1 * 4] * (1 - keep)
    return out


def part_mask(rgb, part):
    """The part's outline in the original's pixels: its box, or the cool-coloured pixels in it
    (guards against a warm room), or the strongly warm ones (an orange craft over pink sand), with warm gaps closed (faces, hands) and grown by a pixel."""
    x0b, y0b, x1b, y1b = part["box"]
    mask = np.zeros(rgb.shape[:2], bool)
    sub = rgb[y0b:y1b, x0b:x1b]
    sel = part.get("select")
    mask[y0b:y1b, x0b:x1b] = ((sub[..., 0] - sub[..., 2]) < 30 if sel == "cool" else
                              (sub[..., 0] - sub[..., 2]) > 100 if sel == "warm" else
                              sub.sum(-1) < 330 if sel == "dark" else True)
    if part.get("close"):
        k = int(part["close"])
        mask = cv2.morphologyEx(mask.astype(np.uint8), cv2.MORPH_CLOSE, np.ones((k, k), np.uint8)) > 0
    # grow: a wider outline (low-res pixels) for small sprites whose close-up comes out a
    # little larger than the pixel art (a helmet over a hat), so its edge is not cut off
    return cv2.dilate(mask.astype(np.uint8), np.ones((3, 3), np.uint8), iterations=1 + int(part.get("grow", 0))) > 0


def silhouette(mask, feather=1.5):
    """The outline at 4x, softened by feather (HD pixels), as blend weights."""
    H0, W0 = mask.shape
    sil = cv2.resize(mask.astype(np.float32), (W0 * 4, H0 * 4), interpolation=cv2.INTER_CUBIC)
    return np.clip(cv2.GaussianBlur(sil, (0, 0), feather), 0, 1)[..., None]


def copy_part(hd, rgb, src, part):
    """The same sprite drawn again (moved, mirrored with flip, or larger or smaller: the
    boxes may differ in size, e.g. the same statue nearer the camera): the HD result of the
    source part is reused, so both look alike, and pasted within this part's outline. The
    box may run past the picture's edge; what is outside is cut off."""
    (sx0, sy0, sx1, sy1), (dx0, dy0, dx1, dy1) = src["box"], part["box"]
    H0, W0 = rgb.shape[:2]
    pad = max(0, min(2, sx0, sy0, W0 - sx1, H0 - sy1))
    patch = hd[(sy0 - pad) * 4:(sy1 + pad) * 4, (sx0 - pad) * 4:(sx1 + pad) * 4]
    if part.get("flip"):
        patch = patch[:, ::-1]
    # the patch scaled onto the destination box (with the same padding, scaled)
    kx, ky = (dx1 - dx0) / (sx1 - sx0), (dy1 - dy0) / (sy1 - sy0)
    X0, Y0 = round((dx0 - pad * kx) * 4), round((dy0 - pad * ky) * 4)
    X1, Y1 = round((dx1 + pad * kx) * 4), round((dy1 + pad * ky) * 4)
    patch = cv2.resize(patch, (X1 - X0, Y1 - Y0), interpolation=cv2.INTER_LANCZOS4 if kx > 1 else cv2.INTER_AREA)
    cx0, cy0, cx1, cy1 = max(0, X0), max(0, Y0), min(W0 * 4, X1), min(H0 * 4, Y1)
    clipped = {**part, "box": [max(0, dx0), max(0, dy0), min(W0, dx1), min(H0, dy1)]}
    sil = silhouette(part_mask(rgb, clipped))[cy0:cy1, cx0:cx1]
    out = hd.copy()
    dst = out[cy0:cy1, cx0:cx1]
    dst[:] = patch[cy0 - Y0:cy1 - Y0, cx0 - X0:cx1 - X0] * sil + dst * (1 - sil)
    return out


def is_area(part):
    """A part drawn from a box (the user's annotation: an area to improve, not a cut line),
    as opposed to a sprite cut out by its colours (select: a guard against the room)."""
    return "select" not in part


def area_mask(shape, box, soft):
    """A box as blend weights at 4x: full inside, fading out across its edges over about
    soft low-res pixels each way, so a redone area has no visible border."""
    H0, W0 = shape
    x0, y0, x1, y1 = box
    m = np.zeros((H0 * 4, W0 * 4), np.float32)
    m[y0 * 4:y1 * 4, x0 * 4:x1 * 4] = 1
    return np.clip(cv2.GaussianBlur(m, (0, 0), soft * 4) * 1.7 - 0.07, 0, 1)[..., None]


def area_style(part, st, room=True):
    """The style a part is drawn with: the room's look (its style layers and, with room, its
    own description) under the part's text, so the redone area belongs to the room. The
    close-up leaves the room's description out: it would drown the part's own text."""
    style = st.get("style") if isinstance(st.get("style"), dict) and part.get("style") is not False else None
    layers = (style["layers"] if style else []) + ([st["prompt"]] if room else [])
    return {**(style or {}), "layers": layers, "negative": style.get("negative", "") if style else ""}


def refine(pipe, hd, rgb, part, st):
    """A person or object painted into a room, redone in a close-up and pasted back.
    A sprite (select: its colours) is pasted within its outline, with its own description
    alone. An area (a box) is drawn with the room's description and look under its own text,
    over the room's rendering around it, and blended in with a wide soft edge."""
    x0b, y0b, x1b, y1b = part["box"]
    H0, W0 = rgb.shape[:2]
    area = is_area(part)
    soft = max(2.0, 0.3 * min(x1b - x0b, y1b - y0b)) if area else 0
    cx, cy = (x0b + x1b) // 2, (y0b + y1b) // 2
    ch = max(int((y1b - y0b) * part.get("margin", 1.4)), int(y1b - y0b + 4 * soft))
    cw = max(int(ch * 0.8), int((x1b - x0b) * part.get("margin", 1.4)), int(x1b - x0b + 4 * soft))  # wide boxes stay whole
    x0, y0 = max(0, cx - cw // 2), max(0, cy - ch // 2)
    x1, y1 = min(W0, x0 + cw), min(H0, y0 + ch)
    # An area is drawn near the model's own size (about 1024 px), no closer: a large box
    # in a 12x close-up would come out far more detailed than the room around it.
    z = st.get("zoom", 12) if not area or "zoom" in part else int(min(12, max(4, round(1024 / max(x1 - x0, y1 - y0)))))
    gw, gh = (x1 - x0) * z // 8 * 8, (y1 - y0) * z // 8 * 8
    seen = rgb.copy()
    for dx0, dy0, dx1, dy1, *by in part.get("darken", []):
        # Only in the guide: an object drawn in the same grey as what it stands against
        # (a rifle beside a leg) reads as part of it; darkened, the model sees it apart.
        sub = seen[dy0:dy1, dx0:dx1]
        sub[(sub[..., 0] - sub[..., 2]) < 30] *= by[0] if by else 0.35
    low = guide_of(seen[y0:y1, x0:x1], gw, gh, st.get("soften", 1.5) * z / 4)
    ctx = Image.fromarray(np.clip(hd, 0, 255).astype(np.uint8)).crop((x0 * 4, y0 * 4, x1 * 4, y1 * 4)).resize((gw, gh), Image.LANCZOS)
    if area:
        weights = area_mask((H0, W0), part["box"], soft)
    else:
        weights = silhouette(part_mask(rgb, part), part.get("feather", 1.5))
    crop_w = weights[y0 * 4:y1 * 4, x0 * 4:x1 * 4, 0]
    if area:
        m = Image.fromarray((crop_w * 255).astype(np.uint8)).resize((gw, gh), Image.BILINEAR)
    else:
        m = Image.fromarray((part_mask(rgb, part)[y0:y1, x0:x1] * 255).astype(np.uint8)).resize((gw, gh), Image.BILINEAR).filter(ImageFilter.GaussianBlur(z * 0.6))
    pst = {**st, "prompt": part["prompt"], "negative": ", ".join(filter(None, [part.get("negative_extra"), st["negative"]]))}
    if area:
        pst["style"] = area_style(part, st, room=False)
    elif part.get("style") is False:
        pst["style"] = None
    # context: false draws the part from the pixel art alone, not over the room's own
    # rendering: a character then looks the same whatever the room's settings.
    guide = Image.composite(low, ctx, m) if part.get("context", True) else low
    gen = pipe(guide, pst, part.get("seed", 1992))
    spr = np.asarray(gen.resize(((x1 - x0) * 4, (y1 - y0) * 4), Image.LANCZOS), dtype=np.float32)
    sil = weights[y0 * 4:y1 * 4, x0 * 4:x1 * 4]
    out = hd.copy()
    out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] = spr * sil + out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] * (1 - sil)
    return out


def harmonize(pipe, hd, part, st, strength, keep=None):
    """The surroundings of a redone area repainted gently with the room's own settings and
    description, so the area takes the room's detail, light and sharpness while keeping
    what was redone; blended with a wide soft edge. keep (4x weights): sprites already drawn
    (the guards), left as they are so they stay the same in every room."""
    x0b, y0b, x1b, y1b = part["box"]
    H, W = hd.shape[:2]
    H0, W0 = H // 4, W // 4
    g = int(max(x1b - x0b, y1b - y0b) * 0.6) + 4
    x0, y0, x1, y1 = max(0, x0b - g), max(0, y0b - g), min(W0, x1b + g), min(H0, y1b + g)
    k = 1024 / (max(x1 - x0, y1 - y0) * 4)
    gw, gh = int((x1 - x0) * 4 * k) // 8 * 8, int((y1 - y0) * 4 * k) // 8 * 8
    region = Image.fromarray(np.clip(hd[y0 * 4:y1 * 4, x0 * 4:x1 * 4], 0, 255).astype(np.uint8)).resize((gw, gh), Image.LANCZOS)
    pst = {**st, "prompt": part["prompt"], "strength": strength, "cond": 0.6, "style": area_style(part, st),
           "negative": ", ".join(filter(None, [part.get("negative_extra"), st["negative"]]))}
    gen = np.asarray(pipe(region, pst, part.get("seed", 1992)).resize(((x1 - x0) * 4, (y1 - y0) * 4), Image.LANCZOS), dtype=np.float32)
    grown = [max(0, x0b - g // 3), max(0, y0b - g // 3), min(W0, x1b + g // 3), min(H0, y1b + g // 3)]
    wgt = area_mask((H0, W0), grown, g / 4)[y0 * 4:y1 * 4, x0 * 4:x1 * 4]
    if keep is not None:
        wgt = wgt * (1 - keep[y0 * 4:y1 * 4, x0 * 4:x1 * 4])
    out = hd.copy()
    out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] = gen * wgt + out[y0 * 4:y1 * 4, x0 * 4:x1 * 4] * (1 - wgt)
    return out


def sprite(pipe, rgb, alpha, st):
    """Small art: enlarged so the model works near its native size, then 4x, anchored, original outline."""
    h, w = alpha.shape
    z = int(min(16, max(4, round(1024 / max(w, h)))))
    gw, gh = max(64, w * z // 8 * 8), max(64, h * z // 8 * 8)
    out = pipe(guide_of(rgb, gw, gh, st["soften"] * z / 4), st, st.get("seed", 1992))
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
    out = pipe(guide_of(sq, 1024, 1024, st["soften"]), st, st.get("seed", 1992))
    s = 1024 / side
    return np.asarray(out.crop((round(x0 * s), round(y0 * s), round((x0 + w) * s), round((y0 + h) * s))), dtype=np.float32)


# ---------- batch ----------

def key_of(work, it, method, st, models):
    src = os.path.join(work, it["original"]["path"])
    h = hashlib.sha1(json.dumps({"method": method, "st": st, "models": models, "src": os.path.getsize(src)}, sort_keys=True).encode())
    h.update(open(src, "rb").read())
    for name in sorted({p.get("emblem") or p["banner"] for p in st.get("parts", []) if "emblem" in p or "banner" in p}):
        h.update(open(emblem_file(name), "rb").read())
    return h.hexdigest()[:10]


def with_override(st, override):
    """A recipe's settings with the studio's "Try settings" on top (detail: merged; null: off)."""
    st = dict(st)
    for k, v in (override or {}).items():
        if k == "detail" and isinstance(v, dict) and isinstance(st.get("detail"), dict):
            st["detail"] = {**st["detail"], **v}
        elif v is None:
            st.pop(k, None)
        else:
            st[k] = v
    return st


TRY_KEYS = ("prompt", "negative", "strength", "cond", "soften", "seed", "anchor", "detail", "model")


def recipe_info(a):
    """The settings the studio's "Try settings" starts from, for one item."""
    rec = Recipes(a.recipes)
    it = next(i for i in json.load(open(os.path.join(a.work, "review", "items.json")))["items"] if i["id"] == a.id)
    method, st = rec.recipe(it)
    bases = [{"id": m["id"], "label": m.get("label", m["repo"]), "optional": bool(m.get("optional"))} for m in rec.downloads.values() if m.get("base")]
    if method == "recipe-head":  # generate.py: its settings, its description as the prompt
        ch = rec.heads["characters"][st["head"]]
        hs = {**rec.heads.get("defaults", {}), **ch.get("settings", {})}
        model = next((m["id"] for m in rec.downloads.values() if m["repo"] == hs.get("model")), None)
        print(json.dumps({"method": method, "source": st["source"], "models": bases,
                          "settings": {"prompt": ch["description"], "strength": hs.get("strength"), "cond": hs.get("cond"),
                                       "soften": hs.get("soften"), "seed": hs.get("seed"), "model": model}}))
        return
    bases = [{"id": m["id"], "label": m.get("label", m["repo"]), "optional": bool(m.get("optional"))} for m in rec.downloads.values() if m.get("base")]
    default = next((m["id"] for m in rec.downloads.values() if m["repo"] == rec.models["model"]), None)
    print(json.dumps({"method": method, "source": st.get("source"), "parts": len(st.get("parts", [])), "models": bases,
                      "settings": {k: st.get(k) for k in TRY_KEYS} | {"seed": st.get("seed", 1992), "model": st.get("model", default)}}))


def save(work, it, pixels, key, method, st, label=None):
    d = os.path.join(work, "versions", it["id"].replace("/", "__"))
    os.makedirs(d, exist_ok=True)
    mode = "RGBA" if pixels.shape[-1] == 4 else "RGB"
    Image.fromarray(np.clip(pixels, 0, 255).astype(np.uint8), mode).save(os.path.join(d, f"batch-{key}.png"))
    meta = {"label": label or f"batch ({st.get('source')}) {time.strftime('%Y-%m-%d')}",
            "settings": {k: v for k, v in st.items() if k not in ("parts", "part")} | {"method": method, "parts": len(st.get("parts", []))}}
    json.dump(meta, open(os.path.join(d, f"batch-{key}.json"), "w"), indent=2)


def batch(a):
    global DUNE_HD
    work = os.path.abspath(a.work)
    rec = Recipes(a.recipes)
    EMBLEM_DIRS[:] = [os.path.join(work, "emblems"), os.path.join(a.recipes, "emblems")]
    DUNE_HD = a.dune_hd
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
        if it["kind"] == "head-group" and it["group"] not in rec.heads["characters"]:
            print(f"!! {it['id']}: a talking group needs its head's recipe (recipes/heads.json)", flush=True)
            continue
        method, st = rec.recipe(it)
        if method == "recipe-head":
            if a.override:
                print(f"!! {it['id']}: Try settings do not apply to recipe heads (recipes/heads.json)", flush=True)
                continue
            recipe_heads.setdefault(st["head"], []).append(it)
            continue
        if a.override:
            st = with_override(st, json.loads(a.override))
            if st.get("model") in rec.downloads:  # a model id from models.json: its repository
                st["model"] = rec.downloads[st["model"]]["repo"]
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
            save(work, it, px, key, method, st, a.label)
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
    b.add_argument("--override", help="JSON settings over the recipe (the studio's Try settings): prompt, strength, cond, soften, seed, detail…")
    b.add_argument("--label", help="label of the versions made")
    r = sub.add_parser("recipe", help="an item's current settings as JSON")
    r.add_argument("work")
    r.add_argument("--id", required=True)
    r.add_argument("--recipes", default=os.path.join(HERE, "..", "..", "recipes"))
    a = ap.parse_args()
    if a.cmd == "recipe":
        a.recipes = os.path.abspath(a.recipes)
        return recipe_info(a)
    a.recipes = os.path.abspath(a.recipes)
    sys.exit(batch(a) or 0)


if __name__ == "__main__":
    main()
