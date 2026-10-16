"""Generative HD talking heads: photo-like portraits made from the game's faces.

    python generate.py NAME [--heads DIR] [--out DIR] [--only f01,v04] [--force]
                       [--prompt TEXT] [--negative TEXT] [--var-prompt TEXT]
                       [--strength S] [--cond C] [--soften B] [--seed N]

NAME is a head described in recipes/heads.json (LETO, PAUL...). --heads is a
`dune-hd heads` dump (NAME-fNN.png: the frames the game lists; NAME-vNN.png:
part group NN drawn over frame 0). The default frame is generated with SDXL
img2img guided by a tile ControlNet on a softened guide; each variant listed
for the character is then made from it:
- "rerender" (shapes: mouths, brows, a tired face): the same generation, same
  seed, on the variant's frame, kept only where that frame differs;
- "recolour" (colours: Paul's eye stages): no generation, see recolour.py.
Every talking group of the head (talking_groups: the mouths and eyes the game
swaps in while the character talks) is a rerender variant too, with the
character's settings, unless the recipe gives it its own entry; without
--only, all of them are made. The character's "mouths" entry (var_prompt,
negative, settings) applies to every mouth group (mouth_groups) without an
entry of its own, so all mouths share one look.
A variant's "drop" lists phrases of the character's description and negative
it leaves out.
A character with "style": true is generated with the game's style layers
(recipes/style.json: the game's, then its location's), encoded as hdgen.py
does, so it matches a portrait first made by hdgen.py's batch (class
defaults: description "the subject as drawn").
Emblems (the character's "emblems": box in the frame's pixels, emblem name) are
taken out of every frame before generation and drawn back from their vector
master (NAME.svg, as in hdgen.py) on every final frame, so they are exact and
the same on all of them.
Renders are cached in OUT/render with the exact settings beside them, so a
rerun only renders what changed. The options override the character's
settings, for experiments (use another --out). See docs/generative-portraits.md.
"""
import argparse
import json
import os
import time

import numpy as np
from PIL import Image, ImageFilter

import device
import recolour as rc

HERE = os.path.dirname(os.path.abspath(__file__))
TEMPLATE = ("photograph, head and shoulders portrait of {}, warm dramatic side light, dark background, "
            "85mm lens, realistic skin texture, sharp focus")
NEGATIVE = ("cartoon, anime, painting, illustration, drawing, pixel art, cgi, 3d render, blurry, deformed, "
            "disfigured, extra limbs, text, watermark, frame")
FACIAL_HAIR = "mustache, moustache, beard, stubble, facial hair"
BACKDROP = (24, 16, 12)


def load_face(path):
    """The face on a dark backdrop, padded to a square (at the bottom); and its opaque mask."""
    im = Image.open(path).convert("RGBA")
    side = max(im.size)
    sq = Image.new("RGBA", (side, side), BACKDROP + (255,))
    off = ((side - im.width) // 2, side - im.height)
    sq.alpha_composite(im, off)
    mask = Image.new("L", (side, side), 0)
    mask.paste(im.split()[3], off)
    return sq.convert("RGB"), mask


def up(img, soften, size):
    """Bicubic to size, then blurred so the pixel stair-steps don't read as content."""
    big = img.resize((size, size), Image.BICUBIC)
    return big.filter(ImageFilter.GaussianBlur(soften)) if soften > 0 else big


def diff_mask(low, vlow, size):
    """Feathered mask of where a variant differs from the default frame; None if it doesn't."""
    diff = np.abs(np.asarray(vlow, dtype=np.int16) - np.asarray(low, dtype=np.int16)).sum(-1) > 0
    if not diff.any():
        return None
    m = Image.fromarray((diff * 255).astype(np.uint8)).filter(ImageFilter.MaxFilter(5))
    m = m.resize((size, size), Image.BILINEAR).filter(ImageFilter.GaussianBlur(10))
    return Image.fromarray(np.minimum(np.asarray(m, dtype=np.float32) * 2.5, 255).astype(np.uint8))


def match_colours(full, base, mask):
    """The variant's render with the base's colour statistics where it will be blended in:
    another prompt (a mouth's) can shift the skin tone, which would flicker when talking."""
    m = np.asarray(mask, dtype=np.float32)[..., None] / 255
    a, b = np.asarray(full, dtype=np.float32), np.asarray(base, dtype=np.float32)
    w = m.sum()
    if w < 1:
        return full
    ma, mb = (a * m).sum((0, 1)) / w, (b * m).sum((0, 1)) / w
    sa = np.sqrt((((a - ma) ** 2) * m).sum((0, 1)) / w) + 1e-3
    sb = np.sqrt((((b - mb) ** 2) * m).sum((0, 1)) / w)
    return Image.fromarray(np.clip((a - ma) / sa * sb + mb, 0, 255).astype(np.uint8))


def shrink_error(out, low, mask):
    """Mean RGB distance between the result shrunk back and the original (face pixels)."""
    small = np.asarray(out.resize(low.size, Image.BOX), dtype=np.float32)
    ref = np.asarray(low, dtype=np.float32)
    m = np.asarray(mask) > 0
    return float(np.sqrt(((small - ref) ** 2).sum(-1))[m].mean())


def without_emblems(low, emblems):
    """The frame (square, RGB) with each emblem's red pixels filled from the cloth around."""
    import cv2
    a = np.asarray(low).astype(np.int16)
    mask = np.zeros(a.shape[:2], np.uint8)
    for e in emblems:
        x0, y0, x1, y1 = e["box"]
        sub = a[y0:y1, x0:x1]
        mask[y0:y1, x0:x1] = (sub[..., 0] - np.maximum(sub[..., 1], sub[..., 2])) > 40
    mask = cv2.dilate(mask, np.ones((3, 3), np.uint8))
    return Image.fromarray(cv2.inpaint(np.asarray(low), mask, 3, cv2.INPAINT_TELEA))


def with_emblems(im, original, emblems):
    """Each emblem drawn from its vector master on the HD frame, in the original's red,
    shaded by the cloth under it (its folds and light) so it reads as printed on it."""
    import cv2
    import hdgen
    out = np.asarray(im, dtype=np.float32).copy()
    s = im.width / original.width
    orig = np.asarray(original).astype(np.float32)
    for e in emblems:
        x0, y0, x1, y1 = e["box"]
        X0, Y0, X1, Y1 = (round(v * s) for v in (x0, y0, x1, y1))
        em = hdgen.render_emblem(e["emblem"], X1 - X0, Y1 - Y0)
        if e.get("flip"):
            em = em[:, ::-1]
        a = em[..., 3:] / 255
        sub = orig[y0:y1, x0:x1]
        red = (sub[..., 0] - np.maximum(sub[..., 1], sub[..., 2])) > 40
        colour = np.array(e["colour"], np.float32) if "colour" in e else np.percentile(sub[red], 75, axis=0)
        region = out[Y0:Y1, X0:X1]
        lum = cv2.GaussianBlur(region @ np.array([0.2126, 0.7152, 0.0722], np.float32), (0, 0), max(1.0, s / 2))
        shade = np.clip(lum / max(1.0, float(np.median(lum))), 0.6, 1.4)[..., None]
        out[Y0:Y1, X0:X1] = region * (1 - a) + colour * shade * a
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8))


def talking_groups(layout):
    """The groups a head swaps into frame 0 while talking (as `dune-hd inventory` lists
    them, head/NAME/vGG): every group outside frame 0, except those building the faces
    listed as frames (the Fremen sheets' hair and beards)."""
    frames = layout["frames"]
    faces = {g for fr in frames if len(fr) >= 2 for g in fr}
    return [g for g in range(len(layout["groups"])) if g not in frames[0] and g not in faces]


def mouth_groups(layout):
    """The talking groups drawn only where the head's lip-sync mouths are: its mouths
    (the others are eyes, brows and gestures)."""
    groups = layout["groups"]
    at = {(p[1], p[2]) for frame in layout.get("lipsync", []) for g in frame for p in groups[g]}
    return {g for g in talking_groups(layout) if at and all((p[1], p[2]) in at for p in groups[g])}


def without(text, drop):
    """`text` (comma-separated phrases) without the phrases in `drop`."""
    return ", ".join(p for p in (q.strip() for q in text.split(",")) if p and p not in drop)


def prompts(ch, var_prompt=None, variant=False, var_negative=None, drop=()):
    """(prompt, negative prompt) for a character's frame; variant_negative only for the
    variants (mouths...), so it never changes the approved default frame. `drop`: phrases
    of the character's description and negative a variant leaves out (e.g. "thin lips"
    on a mouth that shows its teeth)."""
    prompt = TEMPLATE.format(without(ch["description"], drop))
    negative = NEGATIVE
    if ch.get("clean_shaven"):
        negative = f"{FACIAL_HAIR}, {negative}"  # shadow above an open mouth reads as a moustache
    if without(ch.get("negative", ""), drop):
        negative = f"{without(ch['negative'], drop)}, {negative}"
    if variant and ch.get("variant_negative"):
        negative = f"{ch['variant_negative']}, {negative}"
    if var_negative:  # the variant's own (e.g. no bared teeth on an open mouth)
        negative = f"{var_negative}, {negative}"
    if var_prompt:
        prompt = f"{var_prompt}, {prompt}"  # first: CLIP keeps 77 tokens
    return prompt, negative


class Renderer:
    """SDXL img2img + tile ControlNet, loaded on first use."""

    def __init__(self):
        self.pipe = None

    def load(self, st):
        import torch
        from diffusers import AutoencoderKL, ControlNetModel, StableDiffusionXLControlNetImg2ImgPipeline
        t0 = time.time()
        print("@stage loading the model", flush=True)
        dev = device.pick()
        dt = dev["dtype"]
        control = ControlNetModel.from_pretrained(st["controlnet"], torch_dtype=dt)
        vae = AutoencoderKL.from_pretrained(st["vae"], torch_dtype=dt)
        pipe = StableDiffusionXLControlNetImg2ImgPipeline.from_pretrained(st["model"], controlnet=control, vae=vae,
                                                                          torch_dtype=dt, variant="fp16")
        device.configure(pipe, dev)
        self.pipe, self.torch, self.dtype, self.inpaint = pipe, torch, dt, None
        print(f"model loaded in {time.time() - t0:.0f}s", flush=True)

    def __call__(self, low, prompt, negative, st, style=None):
        if self.pipe is None:
            self.load(st)
        guide = up(low, st["soften"], st["size"])
        g = self.torch.Generator("cpu").manual_seed(st["seed"])
        if style:
            # The style layers before the prompt, each 77-token piece encoded on its own (hdgen.Pipe.embed).
            import hdgen
            enc = hdgen.Pipe(None)
            enc.pipe, enc.torch = self.pipe, self.torch
            text = enc.embed(style["layers"] + [prompt], ", ".join(filter(None, [negative, style.get("negative")])))
        else:
            text = {"prompt": prompt, "negative_prompt": negative}
        return self.pipe(**text, image=guide, control_image=guide, strength=st["strength"],
                         num_inference_steps=st["steps"], controlnet_conditioning_scale=st["cond"],
                         guidance_scale=st["guidance"], generator=g).images[0]


def mouth_pass(render, im, low, vfull, mp, st, ch, drop=()):
    """Redo the inside of an open mouth: the dark interior the pixel art draws (no teeth,
    no gums) painted in, then only the mouth repainted by the model in a close-up, guided
    by it (SDXL ControlNet inpainting, sharing the loaded model), pasted back softly."""
    import cv2
    from diffusers import StableDiffusionXLControlNetInpaintPipeline
    if render.pipe is None:
        render.load(st)
    if render.inpaint is None:
        render.inpaint = StableDiffusionXLControlNetInpaintPipeline.from_pipe(render.pipe, torch_dtype=render.dtype)
    a, b = np.asarray(low).astype(np.int16), np.asarray(vfull).astype(np.int16)
    lum = b @ np.array([0.3, 0.59, 0.11])
    inside = ((np.abs(b - a).sum(-1) > 0) & (lum < mp.get("dark", 45))).astype(np.uint8)
    if mp.get("box"):  # where the mouth is (the square frame's pixels, as emblem boxes), when a larger shadow changes too
        x0, y0, x1, y1 = mp["box"]
        keep = np.zeros_like(inside)
        keep[y0:y1, x0:x1] = 1
        inside &= keep
    # The mouth is the largest dark patch; the others are the jaw's and chin's shadows
    # (picked before closing the patch's holes: closing would join them).
    # The mouth is the largest dark patch; the others are the jaw's and chin's shadows
    # (picked before closing the patch's holes: closing would join them).
    # whole: every patch on the mouth's rows (the opening can be split by lighter pixels):
    # it sizes the close-up and bounds the fill.
    n, lab, stats, _ = cv2.connectedComponentsWithStats(inside, connectivity=4)
    whole = inside.copy()
    if n > 1:
        b = 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))
        top, h = stats[b, cv2.CC_STAT_TOP], stats[b, cv2.CC_STAT_HEIGHT]
        keep = [i for i in range(1, n) if i == b or min(top + h, stats[i, 1] + stats[i, 3]) - max(top, stats[i, 1]) >= 0.5 * stats[i, 3]]
        whole = np.isin(lab, keep).astype(np.uint8)
        inside = (lab == b).astype(np.uint8)
    inside = cv2.morphologyEx(inside, cv2.MORPH_CLOSE, np.ones((3, 3), np.uint8))
    ys, xs = np.nonzero(whole)
    if not len(ys):
        return im
    s = im.width / low.width
    pad = mp.get("pad", 6)  # low-res pixels around the interior: the lips
    x0, y0 = max(0, xs.min() - pad), max(0, ys.min() - pad)
    x1, y1 = min(low.width, xs.max() + 1 + pad), min(low.height, ys.max() + 1 + pad)
    X0, Y0, X1, Y1 = (round(v * s) for v in (x0, y0, x1, y1))
    zoom = 1024 / max(X1 - X0, Y1 - Y0)
    gw, gh = int((X1 - X0) * zoom) // 8 * 8, int((Y1 - Y0) * zoom) // 8 * 8
    big = lambda m, blur: cv2.GaussianBlur(cv2.resize(m.astype(np.float32), (im.width, im.height), interpolation=cv2.INTER_LINEAR), (0, 0), blur)
    dark = np.clip(big(inside, s * 0.5), 0, 1)[Y0:Y1, X0:X1, None]
    grow = cv2.dilate(inside, np.ones((3, 3), np.uint8), iterations=mp.get("grow", 1))
    redo = np.clip(big(grow, s * 0.8) * 1.5, 0, 1)[Y0:Y1, X0:X1]
    crop = np.asarray(im, dtype=np.float32)[Y0:Y1, X0:X1]
    colour = np.array(mp.get("colour", [34, 12, 8]), np.float32)
    painted = crop * (1 - dark) + colour * dark
    guide = Image.fromarray(np.clip(painted, 0, 255).astype(np.uint8)).resize((gw, gh), Image.LANCZOS)
    mask = Image.fromarray((redo * 255).astype(np.uint8)).resize((gw, gh), Image.BILINEAR)
    prompt, negative = prompts(ch, mp.get("prompt", "close-up of a man's lips parted while speaking, dark shadow inside the mouth"), drop=drop)
    negative = f"{mp.get('negative', 'teeth, gums, tongue, dentures, lipstick')}, {negative}"
    g = render.torch.Generator("cpu").manual_seed(st["seed"])
    out = render.inpaint(prompt, negative_prompt=negative, image=guide, mask_image=mask, control_image=guide, width=gw, height=gh,
                         strength=mp.get("strength", 0.6), num_inference_steps=st["steps"], guidance_scale=st["guidance"],
                         controlnet_conditioning_scale=mp.get("cond", 0.6), generator=g).images[0]
    rep_ = np.asarray(out.resize((X1 - X0, Y1 - Y0), Image.LANCZOS), dtype=np.float32)
    res = np.asarray(im, dtype=np.float32).copy()
    res[Y0:Y1, X0:X1] = rep_ * redo[..., None] + crop * (1 - redo[..., None])
    if mp.get("fill"):
        # The inside of the mouth as one quiet dark tone (no teeth, no gums to catch the eye),
        # a little of its shading kept. The opening is found in the HD picture itself (the
        # model may open it wider than the pixel art): its dark pixels, closed over the teeth
        # they surround, the largest such patch; inset so the lips stay as drawn.
        region = res[Y0:Y1, X0:X1]
        lum = region @ np.array([0.2126, 0.7152, 0.0722], np.float32)
        mx, mn = region.max(-1), region.min(-1)
        sat = (mx - mn) / np.maximum(mx, 1)
        teeth = (lum > mp.get("teeth_lum", 90)) & (sat < 0.35)  # light and greyish: teeth, not lips or skin
        # Anywhere in the close-up: the model may draw the opening beyond the pixel art's.
        near = cv2.dilate(whole, np.ones((3, 3), np.uint8), iterations=2)
        my = np.nonzero(inside)[0]  # the mouth's rows: not the jaw's shadow below its corner
        near[: max(0, my.min() - 2)] = 0
        near[my.max() + 3:] = 0
        near = cv2.resize(near, (im.width, im.height), interpolation=cv2.INTER_NEAREST)[Y0:Y1, X0:X1]
        hole = (((lum < mp.get("fill_dark", 70)) | teeth) & (near > 0)).astype(np.uint8)
        k = max(3, int(s * mp.get("fill_close", 3.0)) | 1)
        hole = cv2.morphologyEx(hole, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
        # Cut thin links to neighbouring shadows (the jaw's), then keep the mouth.
        ko = max(3, int(s * mp.get("fill_open", 2.0)) | 1)
        hole = cv2.morphologyEx(hole, cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (ko, ko)))
        n, lab, stats, _ = cv2.connectedComponentsWithStats(hole, connectivity=8)
        if n > 1:
            hole = (lab == 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))).astype(np.uint8)
            filled = hole.copy()
            ff = np.zeros((hole.shape[0] + 2, hole.shape[1] + 2), np.uint8)
            cv2.floodFill(filled, ff, (0, 0), 1)
            hole = hole | (1 - filled)  # the holes inside it (teeth) too
            hole = cv2.erode(hole, np.ones((3, 3), np.uint8), iterations=max(1, int(s / 3)))
            a = np.clip(cv2.GaussianBlur(hole.astype(np.float32), (0, 0), s * 0.4), 0, 1)[..., None] * mp.get("fill_opacity", 0.92)
            sel = hole > 0
            dark_px = region[sel & (lum <= np.percentile(lum[sel], 40))] if sel.any() else region.reshape(-1, 3)
            tone = dark_px.mean(0) * mp.get("fill_shade", 0.8)
            res[Y0:Y1, X0:X1] = region * (1 - a) + (tone * 0.85 + region * 0.15) * a
            if os.environ.get("MOUTH_DEBUG"):
                dbg = region.copy(); dbg[hole > 0] = dbg[hole > 0] * 0.4 + np.array([0, 255, 0]) * 0.6
                dbg[(redo > 0.1) & (hole == 0)] = dbg[(redo > 0.1) & (hole == 0)] * 0.7 + np.array([0, 0, 255]) * 0.3
                Image.fromarray(np.clip(dbg, 0, 255).astype(np.uint8)).save(os.environ["MOUTH_DEBUG"])
    return Image.fromarray(np.clip(res, 0, 255).astype(np.uint8))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("name", help="head in recipes/heads.json, e.g. PAUL")
    ap.add_argument("--characters", default=os.path.join(HERE, "..", "..", "recipes", "heads.json"))
    ap.add_argument("--heads", default=os.environ.get("DUNE_HEADS", "heads"), help="`dune-hd heads` output (WORK/heads)")
    ap.add_argument("--out", help="output folder (default out/NAME)")
    ap.add_argument("--only", help="only these variants (and what they are made over), e.g. f01,v04")
    ap.add_argument("--force", action="store_true", help="render again even if a cached render has the same settings")
    ap.add_argument("--publish", help="studio work folder: also put each frame there as a new version of head/NAME/fNN")
    ap.add_argument("--label", help="label of the published versions (default: the settings)")
    ap.add_argument("--prompt", help="replace the character's description")
    ap.add_argument("--negative", help="replace the character's extra negative prompt")
    ap.add_argument("--var-prompt", help="replace the prompt prefix of the selected rerender variants")
    ap.add_argument("--emblems", action="append", help="folder of emblem masters (NAME.svg); default: the work folder's emblems (--publish, or the one holding --heads), then recipes/emblems")
    ap.add_argument("--dune-hd", help="dune-hd, to render the emblems (default: target/release)")
    ap.add_argument("--model", help="another base model: a repository, or an id from recipes/models.json")
    ap.add_argument("--no-emblems", action="store_true", help="ignore the character's emblems (the frame as the game draws it)")
    for k, t in (("strength", float), ("cond", float), ("soften", float), ("seed", int)):
        ap.add_argument(f"--{k}", type=t)
    args = ap.parse_args()

    chars = json.load(open(args.characters))
    name = args.name.upper()
    ch = dict(chars["characters"][name])
    if args.prompt:
        ch["description"] = args.prompt
    if args.negative is not None:
        ch["negative"] = args.negative
    st = {**chars["defaults"], **ch.get("settings", {})}
    st.update({k: getattr(args, k) for k in ("strength", "cond", "soften", "seed") if getattr(args, k) is not None})
    if args.model:
        models = json.load(open(os.path.join(os.path.dirname(os.path.abspath(args.characters)), "models.json")))["downloads"]
        st["model"] = next((m["repo"] for m in models if m["id"] == args.model), args.model)
    out = args.out or os.path.join("out", name)
    os.makedirs(os.path.join(out, "render"), exist_ok=True)
    emblems = [] if args.no_emblems else ch.get("emblems", [])
    if emblems:
        import hdgen
        # The work folder's emblems (--publish, else the folder holding --heads), then the recipes'.
        work = args.publish or os.path.dirname(os.path.abspath(args.heads))
        hdgen.EMBLEM_DIRS[:] = args.emblems or [os.path.join(work, "emblems"),
                                                os.path.join(os.path.dirname(os.path.abspath(args.characters)), "emblems")]
        hdgen.DUNE_HD = args.dune_hd

    style = None
    if ch.get("style"):
        import hdgen
        style = hdgen.Recipes(os.path.dirname(os.path.abspath(args.characters))).style_for({"group": name})

    def frame(fid, keep_emblems=False):
        low, mask = load_face(os.path.join(args.heads, f"{name}-{fid}.png"))
        return (low if keep_emblems or not emblems else without_emblems(low, emblems)), mask
    variants = dict(ch.get("variants", {}))
    layout_path = os.path.join(args.heads, f"{name}.json")
    if os.path.exists(layout_path):
        layout = json.load(open(layout_path))
        mouths = mouth_groups(layout)
        for g in talking_groups(layout):
            fid = f"v{g:02d}"
            if fid not in variants and os.path.exists(os.path.join(args.heads, f"{name}-{fid}.png")):
                variants[fid] = {"kind": f"mouth, group {g}", **ch.get("mouths", {})} if g in mouths else {"kind": f"group {g}"}
    only = set(args.only.split(",")) if args.only else set(variants)
    base_id = ch.get("base", "f00")
    needed = only | {variants[v]["over"] for v in only if v in variants and "over" in variants[v]}
    render = Renderer()
    report = {"character": name, "settings": st, "frames": {}}

    todo = [base_id] + [f for f, v in variants.items() if v.get("method", "rerender") == "rerender" and f in needed]
    print(f"@plan {len(todo)} renders", flush=True)

    def cached(fid, low, var_prompt, vst):
        """The full generation for frame `fid`, from the cache when its settings match."""
        v = variants.get(fid, {})
        prompt, negative = prompts(ch, var_prompt, variant=fid != base_id, var_negative=v.get("negative"), drop=v.get("drop", ()))
        params = {"frame": fid, "prompt": prompt, "negative": negative, **vst, **({"emblems": emblems} if emblems else {}), **({"style": style} if style else {})}
        path = os.path.join(out, "render", f"{name}-{fid}.png")
        side = path[:-4] + ".json"
        if not args.force and os.path.exists(path) and os.path.exists(side) and json.load(open(side)) == params:
            return Image.open(path).convert("RGB")
        t = time.time()
        print(f"@stage render {name}-{fid} ({todo.index(fid) + 1 if fid in todo else 1}/{len(todo)})", flush=True)
        im = render(low, prompt, negative, vst, style)
        im.save(path)
        json.dump(params, open(side, "w"), indent=2)
        print(f"rendered {name}-{fid} in {time.time() - t:.0f}s", flush=True)
        return im

    low, mask = frame(base_id)
    base = cached(base_id, low, None, st)
    finals = {base_id: base}

    # Shapes (mouths, brows, a tired face): the same generation on the variant's
    # frame, kept only where that frame differs from the default one.
    for fid, v in variants.items():
        if v.get("method", "rerender") != "rerender" or fid not in needed:
            continue
        vlow, _ = frame(fid)
        if vlow.size != low.size:
            print(f"{fid}: drawn beyond {base_id}'s frame ({vlow.size} against {low.size}), skipped", flush=True)
            continue
        vst = {**st, **v.get("settings", {})}
        if vst.get("open") is not None:
            # A mouth less open than drawn: its guide between the default frame and its own.
            vlow = Image.blend(low, vlow, vst["open"])
        m = diff_mask(low, vlow, st["size"])
        if m is None:
            print(f"{fid}: same as {base_id}, skipped")
            continue
        var_prompt = args.var_prompt if args.var_prompt is not None and fid in only else v.get("var_prompt")
        full = cached(fid, vlow, var_prompt, vst)
        finals[fid] = Image.composite(match_colours(full, base, m), base, m)
        if v.get("mouth_pass"):
            finals[fid] = mouth_pass(render, finals[fid], low, frame(fid)[0], v["mouth_pass"], st, ch, drop=v.get("drop", ()))

    # Colours (eye stages): recoloured from the frame they're drawn over.
    groups = {}
    for fid, v in variants.items():
        if v.get("method") == "recolour":
            groups.setdefault(v.get("over", base_id), []).append(fid)
    for over, ids in groups.items():
        if not any(i in only for i in ids):
            continue
        olow, _ = frame(over)
        stages = [frame(i)[0] for i in ids]
        region = rc.eye_region(olow, stages)
        # The model picks its own iris colour: start from the frame's.
        face = Image.fromarray(rc.recolour(rc.Eye(finals[over], region), olow, olow, region, full=True))
        finals[over] = face
        eye = rc.Eye(face, region)
        for fid, stage in zip(ids, stages):
            if fid in only:
                finals[fid] = Image.fromarray(rc.recolour(eye, olow, stage, region))

    if emblems:
        finals = {fid: with_emblems(im, frame(fid, keep_emblems=True)[0], emblems) for fid, im in finals.items()}
    for fid, im in finals.items():
        if fid != base_id and fid not in needed:
            continue
        im.save(os.path.join(out, f"{name}-{fid}.png"))
        flow, fmask = frame(fid, keep_emblems=True)
        report["frames"][fid] = {"kind": variants.get(fid, {}).get("kind", "default"), "shrink_error": round(shrink_error(im, flow, fmask), 1)}
    json.dump(report, open(os.path.join(out, "report.json"), "w"), indent=2)
    print(f"{len(report['frames'])} frames in {out}")
    if args.publish:
        publish(args.publish, name, {f: finals[f] for f in report["frames"]}, args.heads, args.label, st, report)


def publish(work, name, frames, heads, label, st, report):
    """Each frame as a new version of the studio's head/NAME/fNN item: the
    square generation cropped back to the frame (padded at the bottom, centred)."""
    stamp = time.strftime("%Y%m%d-%H%M%S")
    for fid, im in frames.items():
        w, h = Image.open(os.path.join(heads, f"{name}-{fid}.png")).size
        side = max(w, h)
        s = im.width / side
        x0, y0 = (side - w) // 2, side - h
        crop = im.crop((round(x0 * s), round(y0 * s), round((x0 + w) * s), round((y0 + h) * s)))
        d = os.path.join(work, "versions", f"head__{name}__{fid}")
        os.makedirs(d, exist_ok=True)
        crop.save(os.path.join(d, f"{stamp}.png"))
        meta = {"label": label or f"{stamp} (recipes/heads.json)", "settings": {**{k: st[k] for k in ("strength", "cond", "soften", "seed", "model")}, **report["frames"][fid]},
                "default": not label or not label.startswith("try")}
        json.dump(meta, open(os.path.join(d, f"{stamp}.json"), "w"), indent=2)
    print(f"published {len(frames)} versions to {work}/versions", flush=True)


if __name__ == "__main__":
    main()
