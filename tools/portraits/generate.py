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


def shrink_error(out, low, mask):
    """Mean RGB distance between the result shrunk back and the original (face pixels)."""
    small = np.asarray(out.resize(low.size, Image.BOX), dtype=np.float32)
    ref = np.asarray(low, dtype=np.float32)
    m = np.asarray(mask) > 0
    return float(np.sqrt(((small - ref) ** 2).sum(-1))[m].mean())


def prompts(ch, var_prompt=None):
    """(prompt, negative prompt) for a character's frame."""
    prompt = TEMPLATE.format(ch["description"])
    negative = NEGATIVE
    if ch.get("clean_shaven"):
        negative = f"{FACIAL_HAIR}, {negative}"  # shadow above an open mouth reads as a moustache
    if ch.get("negative"):
        negative = f"{ch['negative']}, {negative}"
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
        self.pipe, self.torch = pipe, torch
        print(f"model loaded in {time.time() - t0:.0f}s", flush=True)

    def __call__(self, low, prompt, negative, st):
        if self.pipe is None:
            self.load(st)
        guide = up(low, st["soften"], st["size"])
        g = self.torch.Generator("cpu").manual_seed(st["seed"])
        return self.pipe(prompt, negative_prompt=negative, image=guide, control_image=guide, strength=st["strength"],
                         num_inference_steps=st["steps"], controlnet_conditioning_scale=st["cond"],
                         guidance_scale=st["guidance"], generator=g).images[0]


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
    out = args.out or os.path.join("out", name)
    os.makedirs(os.path.join(out, "render"), exist_ok=True)
    frame = lambda fid: load_face(os.path.join(args.heads, f"{name}-{fid}.png"))
    variants = ch.get("variants", {})
    only = set(args.only.split(",")) if args.only else set(variants)
    base_id = ch.get("base", "f00")
    needed = only | {variants[v]["over"] for v in only if v in variants and "over" in variants[v]}
    render = Renderer()
    report = {"character": name, "settings": st, "frames": {}}

    todo = [base_id] + [f for f, v in variants.items() if v.get("method", "rerender") == "rerender" and f in needed]
    print(f"@plan {len(todo)} renders", flush=True)

    def cached(fid, low, var_prompt, vst):
        """The full generation for frame `fid`, from the cache when its settings match."""
        prompt, negative = prompts(ch, var_prompt)
        params = {"frame": fid, "prompt": prompt, "negative": negative, **vst}
        path = os.path.join(out, "render", f"{name}-{fid}.png")
        side = path[:-4] + ".json"
        if not args.force and os.path.exists(path) and os.path.exists(side) and json.load(open(side)) == params:
            return Image.open(path).convert("RGB")
        t = time.time()
        print(f"@stage render {name}-{fid} ({todo.index(fid) + 1 if fid in todo else 1}/{len(todo)})", flush=True)
        im = render(low, prompt, negative, vst)
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
        m = diff_mask(low, vlow, st["size"])
        if m is None:
            print(f"{fid}: same as {base_id}, skipped")
            continue
        var_prompt = args.var_prompt if args.var_prompt is not None and fid in only else v.get("var_prompt")
        full = cached(fid, vlow, var_prompt, {**st, **v.get("settings", {})})
        finals[fid] = Image.composite(full, base, m)

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

    for fid, im in finals.items():
        if fid != base_id and fid not in needed:
            continue
        im.save(os.path.join(out, f"{name}-{fid}.png"))
        flow, fmask = frame(fid)
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
        meta = {"label": label or f"{stamp} (recipes/heads.json)", "settings": {**{k: st[k] for k in ("strength", "cond", "soften", "seed")}, **report["frames"][fid]}, "default": True}
        json.dump(meta, open(os.path.join(d, f"{stamp}.json"), "w"), indent=2)
    print(f"published {len(frames)} versions to {work}/versions", flush=True)


if __name__ == "__main__":
    main()
