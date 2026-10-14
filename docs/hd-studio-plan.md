# Plan: Dune HD Studio, an automated generator for HD packs

**Status: plan, 2026-10-14.** Companion to [generative-portraits.md](generative-portraits.md), which describes the portrait pipeline. This plan generalises it to all of the game's art.

## Goal

Anyone with their own `DUNE.DAT` and a Windows or Linux PC with an NVIDIA GPU makes a complete HD pack themselves: **characters, rooms, scenes, sprites and the interface**.

1. Download one program and start it.
2. It sets up everything it needs itself: Python, PyTorch with CUDA, the models.
3. **1-click mode:** pick `DUNE.DAT`, press **Generate**. The program follows our guidelines (recipes, templates, model choice), checks its own results, and writes a pack the app imports.
4. **Advanced mode:** look at every asset before packaging: compare with the pixel art, play animations, regenerate, adjust, approve.

We never ship assets. We ship code and **recipes**: our text, numbers and choices. The pictures are made on the user's computer from their game files.

While we build the first packs by hand and inspect them together, every choice we confirm goes into the recipes. So the 1-click mode improves with each recipe release, without a new program.

## Prototype status (2026-10-14)

A first studio runs today as a local Python server ([`tools/studio`](../tools/studio/README.md)), ahead of the Rust program planned below:
- **Inventory:** `dune-hd inventory` lists the DAT's art as 1,254 review units, in true colours where known. The units are 206 talking-head frames, 27 rooms, 72 scenes and pictures, 61 characters and animated sprites, 478 interface pieces, 242 room pieces, 152 text art and 16 others. Sorting and palettes come from [`recipes/assets.json`](../recipes/assets.json); 146 sprites' colours are not yet certain (mostly the intro subtitles).
- **Recipe bundle v0.1:** [`recipes/`](../recipes/README.md) holds the heads, scenes, rooms (with the throne room's guard parts), interface, models and assets files.
- **Setup:** the studio detects an NVIDIA GPU, Apple's GPU or none; installs the Python environment with the matching PyTorch build, checking disk space first; and downloads the models. Generation picks the GPU and sizes memory use to its VRAM.
- **Jobs:** one at a time, with progress read from the tools' output. Talking heads can be generated from the studio; the result is published as a new version. Through the studio, Paul's eye stage 1 came out identical to the approved frame.
- **Review:** comparison modes, detail map, boxes and notes, saved to `review.json`. `review_todo.py` summarises them for the next round, run by a person or by Claude through the same CLI.

The CUDA path has not run yet: it waits for the Windows NVIDIA machine (phase 6).

To continue on another computer, see [hd-handoff.md](hd-handoff.md).

## Requirements

| | |
|---|---|
| **Systems** | Windows 10/11 x64 and Linux x64 (glibc 2.28 or later). macOS (Apple Silicon) for our own development, not as a target. |
| **GPU** | NVIDIA, 8 GB of VRAM or more recommended; 6 GB works more slowly (model parts offloaded to the CPU). Only the NVIDIA driver is needed: the PyTorch wheels carry the CUDA runtime. AMD (ROCm on Linux) is a later option. Without a supported GPU, the program offers the classic (non-generative) pack only. |
| **Disk** | About 30 GB: Python and PyTorch (about 4 GB), models (about 10–15 GB, depending on the classes generated), cache and work files. |
| **User steps** | Install nothing by hand. No terminal, no Hugging Face account (no gated models), no Python. |
| **Network** | Only for the first setup and for recipe updates. Nothing is uploaded, ever. |

## What there is to make

`DUNE.DAT` holds 1,947 distinct sprites in 128 sheets. Some art is not in sprites at all: the game's code draws the globe, the map, the sky gradients, transitions and the cursor. Those are outside a pack, as today.

| Class | Sheets (examples) | What it is | Method | Stored as |
|---|---|---|---|---|
| **Talking heads** | `LETO` … `MOIS`, `FRM1`–`FRM3` (19 heads, 206 frames) | Faces assembled from parts: eyes, mouths, overlays | Generative portraits ([described here](generative-portraits.md)). One generation per face; shapes by masked rerender; colours by recolour | True-colour **head plates** and mouth patches |
| **Scenes** | `DN20`–`DN38`, `DS*`, `DV*`, `DH*`, `DF*`, `DP*` (desert views), `VIL*`, `VG*`, `INT*`, `SUN`, `VIS`, `XPLAIN9`… (about 70 full-screen pictures, 320 × 152) | Backgrounds. The desert views are stored **grey**: the game colours them at run time with time-of-day palettes | Generative upscale of the whole picture (tiled diffusion, 4×), made in a reference palette | **Palette-indexed** HD, so day and night palettes still apply (see [Formats](#engine-work-hd-formats)) |
| **Rooms** | `PALAIS`, `SIET1`, `CORR`, `BUNK`, `SERRE`, `EQUI`, `COMM`… → 27 rooms drawn by the game | Rooms assembled from many sprites (existing room plates) | Generative img2img of each whole room. Its states (an open door, objects) by masked rerender, like mouths | Room plates, true colour or indexed by recipe |
| **Characters and animated sprites** | `PERS` (full-body characters in rooms), `ORNY*`, `VER`, `SUNRS`, `CHANKISS`… | Small sprites, often several frames of one animation, drawn over a room or scene | Generative at low strength **in context** (over their room, as head parts already are). Same description as the character's portrait, so Leto stays Leto. Frames by masked rerender with a fixed seed. Classic below a minimum size | True-colour sprite art |
| **Interface** | `ICONES`, `BOOK`, `GENERIC` (digits), `ONMAP`, `ATTACK`, `PALPLAN`, `STARS`, `MIXR`… | Icons, book pages, digits, map markers, masks | Classic upscale with the existing guards (exact edges and silhouettes), optionally a light generative pass on ornamental art (`BOOK`). Masks and digits stay classic | Palette-indexed (as today), so highlights and palette effects keep working |
| **Text art** | `IRUL*` (intro subtitles) | Text pre-rendered on its background | Built-in MMPX (existing rule) | Built-in |

The sheet-to-class table goes into the recipe bundle (`assets.json`). It is first filled automatically:
- a head layout means a talking head;
- a single 320 × 152 sprite means a scene;
- a room drawn by the harness means a room;
- small, flat or two-colour sprites mean interface or text.

We then confirm it while reviewing.

**Consistency, beyond faces:**
- **Series.** The same view or object appears several times: desert views at different places, room states, animation frames. A series uses one recipe and one seed. Its members are made from each other with masks where they overlap, not generated separately.
- **Same character across classes.** A character's portrait, room sprite and scene appearances share the description from their recipe. A reference-image adapter (IP-Adapter) taking the approved portrait is an option if text alone doesn't hold the identity.
- **One style for the whole game.** A style guide in the bundle (lighting, materials, how photographic or painterly) is used by every class's prompts, so rooms, faces and landscapes look like one game.

## Architecture

```
┌──────────────── dune-hd-studio (one Rust executable) ─────────────────┐
│ local web server (127.0.0.1, random port) ── opens the browser ──► UI   │
│                                                                         │
│  setup        GPU check, disk check, download uv → Python → wheels       │
│               (locked, with hashes) → models (pinned revisions, SHA-256) │
│  game data    DUNE.DAT check, extraction (hdpack crates: heads, sprites, │
│               scenes, rooms via the room harness)                       │
│  jobs         queue per class, cache, resume, progress events → UI       │
│  QA           automatic checks, fallbacks                               │
│  packaging    pack ZIP (pack crate), validated by the engine's reader   │
└───────────────┬─────────────────────────────────────────────────────────┘
                │ JSON lines over stdin/stdout
┌───────────────▼────────────── Python worker (long-lived) ───────────────┐
│ classic upscale · portraits · tiled scenes · rooms · sprites in context │
│ recolour · palette projection · models kept loaded between jobs         │
└─────────────────────────────────────────────────────────────────────────┘
```

**Why this shape:**
- **One Rust executable.** It reuses the crates that already read `DUNE.DAT`, parse heads, run the room harness and write packs. A single file per platform, with no installer needed, though we can also provide one.
- **The UI is a local web page opened in the user's browser**, served by the executable. That avoids a WebView dependency (WebKitGTK on Linux) and gives the same UI on every system. It is built with the web app's stack (Vite + TypeScript), so it can show results through the app's own renderer.
- **The diffusion runs in Python**, where `diffusers` is. The worker is installed and run by the executable through [`uv`](https://github.com/astral-sh/uv), a single static binary that downloads a standalone Python and installs a **locked** environment with hashes. That makes the setup reproducible, needs no admin rights, and keeps everything in the program's own data folder (`%LOCALAPPDATA%\DuneHD` on Windows, `~/.local/share/dune-hd` on Linux). Uninstalling means deleting that folder.
- **The classic upscaler moves into the worker too:** an ESRGAN-type model run with PyTorch, replacing the external command of today's `dune-hd build`. One environment serves every class.
- **The worker stays loaded.** Each model is loaded once per session, and jobs are grouped by model.
- **Considered and not chosen:**
  - **ComfyUI as the backend:** mature, but large, it changes often, it is GPL-3.0, and our own steps (diff masks, recolour, palette projection) would become custom nodes to maintain.
  - **Pure Rust inference (candle, ONNX Runtime):** fewer dependencies, but SDXL plus ControlNet is not supported well enough today. We may revisit it for the classic upscaler first.

## Recipes: the templates we enrich

A **recipe bundle** is a versioned folder (ZIP) with no game data in it:

| File | Holds |
|---|---|
| `bundle.json` | Version, the `DUNE.DAT` versions it supports (hashes), the minimum studio version |
| `models.json` | Models per class: repository, pinned revision, files, SHA-256, licence, why each was chosen |
| `style.md` + `style.json` | The game-wide style guide and the prompt fragments every class uses |
| `assets.json` | Sheet → class, plus overrides per sprite (keep classic, minimum size, series membership) |
| `defaults/<class>.json` | Generation defaults and QA thresholds per class |
| `heads.json` | Talking heads (one entry each): description, negatives, settings, variants, seed list, status |
| `scenes.json`, `rooms.json`, `interface.json` (and later `sprites.json`) | Per scene, room, interface piece or sprite series: description, palette, settings, parts, seeds, status |
| `guidelines.md` | How we write and approve a recipe |

- The first bundle, v0.1, is in [`recipes/`](../recipes/README.md): Leto and Paul approved, Stilgar draft, plus the scene, room and interface pilots.
- Bundles are published as GitHub release assets with checksums. The studio ships one, checks for a newer one on start (it can be turned off), and the pack manifest records which bundle made it.
- **1-click mode generates only `approved` recipes.** Everything else gets the classic upscale, so a partial bundle still gives a complete, better-than-today pack, and every release improves it.
- A CI job validates bundles: schema, references to known sheets, frames and rooms, and no image data.

**The enrichment loop:**
1. We build packs by hand in the advanced mode, class by class.
2. Every change we make (a description, a talking prompt, another seed, a blur, a sprite moved to classic) is kept as a local override.
3. **Export changes** writes the overrides as a patch to the bundle.
4. We review it, approve what we're happy with, and publish a new bundle version.
5. Every user's 1-click mode then follows.

## 1-click mode

**Generate** runs these steps, each resumable (a cache keyed by inputs and settings, as `generate.py` already does):

1. **Check `DUNE.DAT`.** Its hash is matched to the bundle's supported versions. Unknown versions are still allowed, with a warning, using recipes whose ids still match.
2. **Extract** sprites, heads and scenes. The room harness draws the rooms, and a palette trace records the colours scenes are shown in (both exist today: `dune-run --rooms`, `--gfx-trace`).
3. **Classic pass:** every sprite upscaled, as today. This is the complete fallback pack.
4. **Generative passes** for approved recipes, cheapest first so a stopped run still has value:
   1. interface ornaments;
   2. scenes;
   3. rooms;
   4. talking heads;
   5. characters and animated sprites (after the heads, whose approved portraits they can use as references).
5. **Automatic QA** on each result:
   - drift from the pixel art (shrink error, OKLab distance);
   - nothing changed outside a variant's mask;
   - silhouette kept for sprites;
   - plausible eye masks for portraits;
   - a palette-indexed result that still follows the day and night palettes for scenes.

   On failure: retry with the recipe's next seed, then keep the classic art. Each fallback is listed.
6. **Package:** one pack ZIP, checked by loading it with the engine's own pack reader before it is offered.
7. **Report:** what was made, retried or fell back, per class, with the time taken, the models and licences, and the bundle version.

**Volume (estimates):** about 350 portrait generations, about 70 scenes (tiled, several passes each), 27 rooms with their states, and a few hundred sprite generations. Everything else is classic. That's a few hours on a recent NVIDIA GPU, and days on a 16 GB Apple M1. So: progress, time left, pause and resume, and classes can be generated separately.

## Advanced mode (review)

For us first, and for users who want control. Same layout for every class, with a viewer suited to each:
- **Overview:** every asset by class, with status (recipe approved or draft, generated, QA flags, fallback) and a thumbnail; filters by class, status and flag.
- **Viewers:**

  | Class | Viewer |
  |---|---|
  | Talking heads | Pixel art and HD side by side or with a slider; all frames; the talking animation from the lip-sync table; eye stages |
  | Scenes | A time-of-day slider applying the game's own palettes to the HD picture |
  | Rooms | The room with its states; characters and objects drawn over it |
  | Sprites | Animation playback, over their room or scene |
  | Interface | The real screen layout at HD size, so icons and book pages are seen where they sit |

- **Actions:**
  - regenerate with the next seed;
  - edit the recipe (description, prompts, settings) and regenerate one item or a series;
  - compare attempts;
  - approve, or keep the classic art.
- **Preview in the engine:** a viewer page of the web app (a dev route) shows any head, room, scene or sprite through the real compositor and the candidate pack, fades and palettes included. The real game can also be run with the candidate pack.
- **Package**, then **Export changes** to the recipe bundle.

## Phase 0 pilot results (2026-10-14)

| Pilot | Recipe that worked | Notes |
|---|---|---|
| Talking head (Paul) | Strength 0.85, cond 0.6, guide blur 3–4, per-character description and negatives; rerender for shapes, recolour for eyes | See [generative-portraits.md](generative-portraits.md). |
| Scene (`DN20`) | Coloured with `SKYDN` palette 0, 4×, strength 0.85, cond 0.6, blur 4; colour-anchored; stored in palette form | 0.7 is too soft. All 33 time-of-day palettes apply. |
| Room (throne room) | The harness's room in its own palette, 4×, strength 0.85, cond 0.6, blur 4; colour-anchored; stored in palette form (difference 2.4 / 255) | 0.75 keeps the layout but adds little; 0.85 adds carving, candles and fabric without bending the architecture. Fades with the palette. |
| Character in a room (Leto) | A crop around him enlarged 12×, his low-res pixels (blur 1.5) over the HD room; strength 0.9, cond 0.45, negatives against depth of field; kept inside his silhouette; same description as his portrait | At 0.65–0.8 (blur 3) the model keeps him blurred, as an out-of-focus subject. A thin light fringe along the silhouette remains: tighten the alpha edge. |
| Interface (`ICONES`, `BOOK`) | Ornamental pieces (bottom panel `ICONES-000`, book icon `ICONES-009`): coloured with the game palette (from a room capture), enlarged 12×, guide blur 2, strength 0.75, cond 0.7, colour-anchored, palette form, the original silhouette at 4× (transparent windows kept exact) | Carved relief, a real book with text columns, against the classic pack's grainy dithering. 0.55 stays soft. The title picture `BOOK-000` (320 × 150, with the word DUNE) at 0.75 and only 4×: detailed worms and lightning, but a soft background and weathered letters. Treat such pictures as scenes (0.85), and keep the classic art where there is text. Functional pieces (arrows, buttons, masks, digits) stay classic. |

**First pack with generated art (2026-10-14).** `dune-hd build --art DIR` takes finished art (4× pictures, plus an optional palette sidecar) and projects it with the pack's own code. A pack built with the generated throne room, `DN20`, and icons 000 (the bottom panel) and 009 (the book) shows them in the game. In the headless runner's HD screenshots after the intro, the throne room comes from its plate and the panel from the pack. Findings:
- **Sharpness:** the room's architecture gains carving, but it looks softer than the crisp built-in upscale.
- **Room contents drift:** the guards painted into the room lost their rifles and became robed figures. People and objects that are part of a room need describing in its recipe, or masking.
- **Fixed by close-ups:** describing the guards in the room's prompt was not enough, since they are 20 × 35 pixels in a whole-room generation. Redoing each one in a 12× close-up, the way characters are made, works:
  - the close-up uses the guard's own description, strength 0.85, ControlNet 0.5, guide blur 1.5;
  - the guard's outline is taken from the pixel art: cool-coloured pixels against the warm room;
  - the result is pasted back and colour-anchored with the rest of the room.

  The guards now have helmets, armour and rifles, in the pixel art's pose. A sharper room guide (blur 3) and negatives against depth of field also helped a little. Room recipes therefore need a list of **parts** (people and objects painted into the room), each with a box and a description.
- **Leto stays built-in:** character sprites need the true-colour format.

## Engine work: HD formats

Today a pack stores each HD pixel as two palette entries and a blend weight. That is exact for palette effects, but limits an image to the colours around each original pixel. Generated art needs more, in three forms:

1. **Palette-indexed HD (today's form, used for more).** It suits art whose colours come from palette ramps: the grey desert views coloured by time of day, and the interface. **Verified on `DN20` (scene pilot, 2026-10-14):**
   1. The view is coloured with the day palette and generated (4×, strength 0.85, cond 0.6, guide blur 4).
   2. The result is **colour-anchored**: its lightness detail is put back on the original's broad colours, because the model drifts from the game's pink sand to a natural tan.
   3. It is projected onto the palette (two entries found around each original pixel, plus a weight).

   The generated ridges, ripples and mountains survive, and all 33 time-of-day palettes apply exactly. A hybrid (palette form times the HD's own lightness detail) looked almost the same, so the palette form alone is enough for scenes. The desert views' palettes are in `SKYDN.HSQ`: entries 8–40 are 33 palette blocks for colours 73–239 (u16 length, first index, count, 6-bit RGB). `SKY.HSQ` holds 33 more for colours 128–222, for other views.
2. **True-colour art, following fades.** Portraits, rooms and sprites whose generated colours shouldn't be reduced. Each HD pixel keeps the palette index of the low-res pixel under it, and is shown as `art colour × current palette colour ÷ reference palette colour` for that index. That follows fades and tints while keeping the detail. **To verify** on the game's fade-ins and on sprites drawn with several palettes.
3. **Plates for composed pictures.** Head plates are complete faces, keyed by the head resource and the frame's group list. Mouth patches cover one lip-sync group over one face. Room plates (existing) gain true colour and states. The compositor recognises the head sheet's draws as it recognises rooms.

**Pack v4** holds the three forms beside today's art, and the web app reads v2–v4. This is the piece without which nothing generated reaches the game, so it comes first, tested with Paul (true colour) and `DN20` (indexed, already proven offline by the scene pilot).

## Consistency checks and batches on the Windows NVIDIA machine (later)

The Windows machine will be available later. Until then everything runs on the Mac, and this phase is scheduled when it is:
- **Measure.** Run the approved recipes on both machines and compare:
  - on the same machine results are bit-identical (verified on the Mac for Paul);
  - across machines (CUDA versus Apple MPS) and GPU generations they will differ slightly.

  The measurement sets the QA tolerances and tells us how close a user's pack will be to what we approved.
- **Determinism where we can:** noise from a CPU generator (already the case), pinned libraries and model revisions, deterministic CUDA algorithms where they exist.
- **Golden set.** A few recipes per class are rerun on every studio or bundle change, on both machines, and their metrics are compared with the previous run. This is a regression test that stores no images in the repository.
- **Batches.** The same executable has a headless mode (`dune-hd-studio batch …`) for whole-class runs and seed sweeps. We drive it over SSH and bring results back for review. These are our own machines and our own game files; nothing is published.

This phase must be done before the first public release.

## Legal and licences

- **Shipped:** code (Apache-2.0) and recipes (our text and numbers). **Never shipped:** images, masks or anything else derived from the game.
- **Models** are downloaded by the user's program from their publishers. Their licences are shown during setup and recorded in each pack's manifest. Only models whose licence allows this use, and that need no account, are chosen. The licences of RealVisXL V5.0, the xinsir tile ControlNet, the fp16-fix VAE and any classic upscaler or scene model still need checking before the first public release.
- Packs carry the existing personal-use notice.

## Phases

| Phase | Content | Done when |
|---|---|---|
| **0. Foundations and pilots** (Mac) | Recipe bundle v0.1 (`recipes/`: heads, scenes, rooms, interface, `assets.json`; style guide draft). Worker API (JSON lines). **One pilot per class** to settle its method: Paul (done), one desert view with its day and night palettes (done: `DN20`), one palace room (done: the throne room), Leto in that room (done), the `BOOK` and `ICONES` art (done). | Each class has a method that we approved on a real example. |
| **1. HD formats** | Pack v4: true colour with fades, indexed generative art, head plates and mouth patches, room plates in true colour. Compositor matching, web app reader, viewer route. `dune-hd` writes them from the pilots. | The pilots show in the game in HD, through fades and day and night. |
| **2. Studio skeleton** | Rust executable with the local UI; setup wizard (GPU and disk checks, uv, locked environment, models with progress, resume and checksums); `DUNE.DAT` import and check; extraction, room harness and palette trace. | On a clean Linux PC (and Windows when available), setup finishes without a terminal. |
| **3. 1-click** | Job queue per class, cache, pause and resume, classic pass, generative passes, QA and fallbacks, packaging and validation, report. | One click turns `DUNE.DAT` into a working pack. |
| **4. Review** | Overview, a viewer per class, compare, regenerate and edit, approve, preview in the engine, export changes. | We build the next assets entirely in the studio. |
| **5. Content** | Recipes class by class: talking heads (incl. Fremen, mouth openness pre-labelled, Stilgar's eyes), scenes, rooms, characters and sprites, interface ornaments. | Bundle 1.0: every class covered. |
| **6. Windows consistency and batches** (when the machine is available) | Measurement, tolerances, golden set, whole-cast batches. | Tolerances set, golden set green on both machines. |
| **7. Distribution** | Windows ZIP or installer (code signing to decide), Linux AppImage or tarball; recipe updates; user docs. | Someone outside the project makes a pack with the published program. |

Phases 1 and 2 can run in parallel, and so can 4 and 5. Phase 6 can start whenever the Windows machine is available, but must be finished before phase 7.

## Open decisions

| Question | Recommendation |
|---|---|
| UI in the browser or in a desktop window (Tauri)? | Browser: no extra dependency on Linux, same UI everywhere, reuses the web app. |
| Own worker or ComfyUI? | Own worker: smaller, we control it, no GPL, our steps stay plain Python. |
| One model for every class, or one per class? | **Decided (2026-10-14): a separate model per class wherever it gives a better result.** The phase 0 pilots compare candidates per class; `models.json` names one per class, and the worker groups jobs by model so each is loaded once. |
| Keep characters' identity across classes by text only, or with a reference-image adapter? | Text first; add the adapter if the `PERS` pilot shows drift. |
| How close must a user's pack be to what we approved? | Decide after the phase 6 measurement. |
| Store perceptual fingerprints of approved results in the bundle, to check users' results against ours? | Only if phase 6 shows it is needed, and after a legal check. |
| Code signing on Windows | Decide before phase 7. Unsigned works, with a SmartScreen warning. |
| AMD GPUs | Later: ROCm on Linux first. |
