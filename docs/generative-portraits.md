# Generative HD portraits

**Status: prototype.** The tools in [`tools/portraits`](../tools/portraits) make detailed, photo-like portraits of the talking heads (Leto, Paul, Stilgar…) from the game's own faces. Their output is not yet used by HD art packs or by the app: that needs *head plates* (see [Next steps](#next-steps)).

The [HD art packs](../README.md#hd-art-packs) upscale each sprite: they make the art sharper but add no detail. A generative model can paint the detail a 150-pixel-high face only suggests: skin, hair, fabric. The difficulty is that a talking head is not one picture. It is a face with mouths, eyes and expressions drawn over it, and every frame has to stay the same person. This document explains how the game builds its faces, the pipeline that follows from it, and what was tried.

> **⚠️ Never commit or share the results.** Portraits are derived from the game's copyrighted artwork. Generate them from your own `DUNE.DAT`, for your own use. Renders go to `out/` and `heads/` folders, which are ignored by git.

## How the game builds a face

Each talking head (`LETO.HSQ`, `PAUL.HSQ`, …) is a sheet of sprites whose last entry describes how they fit together (parsed by [`crates/hdpack/src/heads.rs`](../crates/hdpack/src/heads.rs)):

- A **group** is a list of sprites with their positions: a face, an eye strip, a mouth, a beard.
- A **frame** is a list of groups drawn in order. The head's frame table lists every frame the game can show, and its last entry is the lip-sync table: the mouth shapes drawn while a character talks.

`dune-hd heads --dat DUNE.DAT --work work` writes, to `work/heads`:
- `NAME-fNN.png`: every frame the game lists;
- `NAME-gNN.png`: every group on its own;
- `NAME-vNN.png`: group NN drawn over frame 0, which is how mouths appear;
- `NAME.json`: the groups (sprite, x, y) and the frames (group lists).

The data answers questions that decide the design:

- **Every face is listed; nothing is combined freely.** The 19 heads have 206 frames in total, 130 of them made of several groups. The three Fremen sheets (`FRM1`–`FRM3`) build 15, 17 and 17 complete faces from shared parts: a base face, hair, beards, moustaches and a robe. Their talking frames are explicit mouth-and-beard pairs (in `FRM1`, 4 mouth shapes, each under one of several beards).
- **Colour changes are separate sprites, not tints.** Paul's eyes turn blue over the game in 9 stages, and each stage is its own eye-strip sprite (grey, then blue whites, then blue iris and pupil). Each stage also exists with and without a tired, gaunt overlay over the whole face: 18 frames.
- **Parts are shared between faces.** One hair sprite appears in 7 Fremen faces.

That last point is why per-sprite replacement, which is how packs work, doesn't fit generated art. A generated face can't be split back into reusable sprites: the same hair would come out with different lighting in each face. Generated portraits have to be stored and shown **per complete face**.

## The pipeline

[`generate.py`](../tools/portraits/generate.py) makes all the frames of one character, from the settings in [`recipes/heads.json`](../recipes/heads.json).

**1. The default face.** Frame `f00` is padded to a square, enlarged to 1024 × 1024 and blurred (the *guide*). SDXL img2img (`RealVisXL_V5.0`, with the `sdxl-vae-fp16-fix` VAE) repaints it, guided by a tile ControlNet (`xinsir/controlnet-tile-sdxl-1.0`) on the same guide:

| Setting | Default | Why |
|---|---|---|
| `strength` | 0.85 | At 0.65 or below the model only repaints the pixel art. At 0.85 it invents real detail and keeps the composition. |
| `soften` | 4 px (Gaussian blur of the guide) | Unblurred, the pixel steps read as content (a grainy beard). Blurred, the guide keeps shapes and colours but not the pixels. Paul uses 3, so the art's shading of brow and cheeks comes through. |
| `cond` | 0.6 | Higher (0.85) makes muddy, closed shapes. |
| seed, steps, guidance | 1992, 30, 6.0 | Fixed, so every frame of a character starts from the same noise. |

The prompt is `photograph, head and shoulders portrait of DESCRIPTION, warm dramatic side light, dark background, 85mm lens, realistic skin texture, sharp focus`. The description, and any extra negative prompt, come from the character's entry. A shrink check measures how far the result, shrunk back to the original size, is from the original (`shrink_error` in `report.json`; about 15–20 is typical).

**2. Every other frame comes from that face, by the kind of change.**

| Change | Examples | Method |
|---|---|---|
| **Shape** | mouths, brows, blinks, Paul's tired face | `rerender`: the same generation (same seed and settings) on the variant's frame, kept only where that frame differs from the default (a feathered mask). Everything else is the default face's pixels, so the frames can't drift apart. |
| **Colour** | Paul's 9 eye stages; Stilgar's blue-within-blue eyes (to do) | `recolour`: no generation; see below. Under a second per frame. |
| **New identity** | each Fremen face | A separate default face (not yet in the tool). |

A rerender sees the same prompt, so it tends to underplay the change: an open mouth comes out barely parted. A `var_prompt` is put in front of the prompt for that variant only, such as `mouth open, speaking, teeth visible` or `exhausted, gaunt hollow cheeks, dark circles under the eyes, worn by the desert`. It goes first because CLIP reads only 77 tokens. It must match the variant: the talking prompt gives teeth to a smirk or to a mouth that is only slightly parted.

**3. Recolouring** ([`recolour.py`](../tools/portraits/recolour.py)). The pixel art says what colour each part of the eye should be at each stage; the generated eye has its own shape and position. So:
1. **Find the eye.** The region any stage recolours in the pixel art, slightly grown, locates the eye in the HD face. Inside it, the eye opening (white, iris, pupil) is what is much less colourful than skin, lids and brows. Morphology removes the thin lash line and fills catchlights. Dark pixels on the edge of the opening are lashes and are left alone; dark pixels inside it are the pupil.
2. **Match by lightness rank, not position.** Each HD eye pixel takes the colour change of the pixel-art eye pixels of the same lightness rank: the white with the white, the iris with the iris, the pupil with the pupil.
3. **Shift lightness, replace hue.** Lightness moves by the stage's change, so the HD texture stays. Hue becomes the stage's own colour, because adding a blue shift to a brown iris makes purple. The change is smoothed inside the eye.
4. **Start from the game's colours.** The model picks its own iris colour (violet-grey for Paul), so the eye is first set to the default frame's colours (grey), then the stages are applied.

**4. Caching.** Each full generation is saved in `OUT/render/` with its exact settings beside it (`NAME-fNN.json`). A rerun reuses a render whose settings haven't changed, so editing one variant re-renders only that one, and an approved character can be rebuilt exactly. `--force` renders again anyway.

## Characters

[`recipes/heads.json`](../recipes/heads.json) (part of the [recipe bundle](../recipes/README.md)) holds the shared defaults and one entry per head:

```jsonc
"PAUL": {
  "status": "approved 2026-10-14 …",            // for people
  "description": "a clean-shaven nobleman, early thirties, lean angular weathered face, …",
  "clean_shaven": true,                          // facial hair in the negative prompt
  "negative": "young, boyish, teenager, baby face, smooth skin, full lips, pink lips, lipstick",
  "settings": {"soften": 3.0},                   // overrides "defaults"
  "variants": {
    "f01": {"kind": "tired", "var_prompt": "exhausted, gaunt hollow cheeks, …"},
    "f02": {"kind": "eyes 1", "method": "recolour"},
    "f03": {"kind": "tired, eyes 1", "method": "recolour", "over": "f01"}
  }
}
```

- **Variant ids** are files of the heads dump: `fNN` when the game lists the frame (Paul's eye stages replace the eye strip, so drawing the new strip over frame 0 would be wrong), `vNN` for a part drawn over frame 0 (mouths).
- **`method`** is `rerender` (the default) or `recolour`. `over` is the frame a recolour starts from (default: the default frame), so Paul's tired frames with blue eyes are the tired face, recoloured.
- **`kind`** is a label for people: `mouth open`, `mouth parted`, `smirk`, `tired`, `eyes 3`…
- **`clean_shaven`**: the shadow above an open mouth reads as a moustache, and the model grows one on some frames and not others (seen on Leto). Forbidding facial hair keeps the frames consistent. Use it only for characters drawn without facial hair.

**Adding a character:** dump the heads, look at `NAME-f*.png` and `NAME-v*.png`, write a description of who is pictured (age, face shape, hair, eyes, clothing: the model follows the pixel art's shapes, and the words decide the rest), list the variants with their kind, then generate the default frame alone (`--only f00`) until it looks right.

## Using it

```sh
# 1. The heads, from your own game files (in the repository root):
cargo build --release
./target/release/dune-hd heads --dat /path/to/DUNE.DAT --work /path/to/portraits-work

# 2. Python with the libraries the prototype was run with (Python 3.14 on an Apple M1):
python3 -m venv venv && venv/bin/pip install -r tools/portraits/requirements.txt

# 3. A character, from a work folder outside the repository. The first run
#    downloads the models (several GB) from Hugging Face; later runs can
#    set HF_HUB_OFFLINE=1.
cd /path/to/portraits-work
python /path/to/dune-hybrid/tools/portraits/generate.py PAUL --heads heads
#  → out/PAUL/PAUL-f00.png … PAUL-f17.png, out/PAUL/report.json
```

**Options:**
- `--only f01,v04` makes only these variants, plus the frames they are made over.
- `--out DIR` writes elsewhere; use it for experiments.
- `--prompt`, `--negative`, `--var-prompt`, `--strength`, `--cond`, `--soften` and `--seed` override the character's settings for one run.
- `recolour.py FACE_HD.png BASE_LOW.png OUT_DIR STAGE_LOW.png…` recolours a face on its own.

**Cost.** On an Apple M1 with 16 GB, a generation takes 8–10 minutes, and up to 25 when the machine swaps. One of 25 denoising steps is about 20 s. Paul's 18 frames need 2 generations (about 20 minutes); a talking head with a few mouths needs 4–5. Runs sometimes die silently just after loading the model under memory pressure: run them again. Wrap long runs in `caffeinate -i` on macOS, or a sleeping Mac stalls them. A CUDA GPU is many times faster, and the script uses one when present.

## What was tried

| Attempt | Result |
|---|---|
| Hard (unblurred) guide, strength ≤ 0.65 | The pixel art repainted, no new detail. |
| Strength 0.85, cond 0.6, blurred guide | **Kept.** Painterly detail, faithful identity. |
| Mouths inpainted over the face | Grew stubble the default face doesn't have; didn't match. |
| Mouths by rerender (same seed, masked) | **Kept.** Consistent, but open mouths read as a moustache on clean-shaven faces → `clean_shaven`. |
| Warping the HD face along the pixel art's motion, then a light refine | Rejected: unnatural crosshatched wrinkles, colour shift. |
| Open mouth: sharper guide (blur 1.5) | Mouth opens, but the beard turns grainy and would flicker. |
| Open mouth: stronger ControlNet (0.85) | Muddy, still closed. |
| Open mouth: a talking prompt for that variant | **Kept** for wide-open mouths; too much for slightly parted ones. |
| Eye stages: colour change placed by pixel position | Magenta on the lids: the generated eye isn't where the pixel-art eye is. |
| Eye stages: segmented opening, matched by lightness rank, hue replaced | **Kept.** |
| Paul younger ("about seventeen") | Too boyish next to the angular pixel art. A new description alone barely changed the face; an anti-youth negative prompt and blur 3 did. |

## Next steps

The automated, UI-based generator these steps lead to is planned in [hd-studio-plan.md](hd-studio-plan.md).

- **Talking prompts per mouth:** label each mouth variant's openness (a mild prompt for parted lips, the talking prompt for open mouths, none for a smirk), ideally estimated from the pixel art (new dark gap or teeth between the lips) and reviewed by a person.
- **Stilgar's blue-within-blue eyes** by recolour, and checking the eye detection on other faces.
- **Fremen:** one default face per listed face (about 50), with their mouth-and-beard patches as rerenders.
- **Head plates in the app.** Like room plates: when the game draws a complete face that the pack has, show the generated portrait, then the mouth patches over it. This needs the pack format and the compositor to key art by the composed frame rather than by sprite.
- **Speed:** a cloud GPU for a full cast; a shorter schedule or a distilled model for iteration.
