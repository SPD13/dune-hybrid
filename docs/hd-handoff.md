# Handoff: generative HD art, Dune HD Studio, and the review loop

**For:** a Claude instance (or a person) continuing this work on another computer, the **Windows PC with an NVIDIA GPU**. Written 2026-10-14 at the end of the macOS sessions. Read this first, then [hd-studio-plan.md](hd-studio-plan.md) (the plan and the pilots' results) and [generative-portraits.md](generative-portraits.md) (the portrait pipeline in depth).

## 1. What we are building

Dune Hybrid plays the original 1992 *Dune* (Cryo) in our own 286 emulator, with an optional **HD art pack**. Today's packs upscale each sprite with a classic upscaler. We are adding **generative** HD art: a diffusion model repaints each picture with real detail (skin, fabric, carved stone, sand ripples) while keeping the pixel art's layout and the game's palette behaviour.

**End goal (the user's words):** "an automated pipeline so that someone with the DAT file can generate in 1-click the HD package as we designed it without us shipping the assets." So:
- We ship **code and recipes**: our prompts, settings and choices, in [`recipes/`](../recipes/README.md). Never images or anything derived from the game.
- The user's own **Dune HD Studio** (Windows or Linux, NVIDIA GPU) downloads the models, generates, checks and packages a pack from their `DUNE.DAT`.
- **One-click mode** follows the approved recipes. **Advanced mode** is the studio's review.
- We **refine the recipes together**: the user reviews and annotates in the studio, Claude turns the annotations into recipe changes and regenerates. The recipes are what ships.

## 2. Standing rules

- **Never commit or share game-derived files:** packs, renders, extracted art, the studio work folder, `DUNE.DAT`, saves. They are for the user's own machines only. `.gitignore` covers the usual folders; check before every commit.
- **Commit or push only when the user asks.** Use the attribution lines the session's system prompt gives.
- **Clean-room rule (engine code):** write the engine only from `DNCDPRG.lst`, the running original (Cryogenic/Spice86) and Cryogenic's sources. Never consult dune-re's game logic. Data-format discoveries from looking at `DUNE.DAT` ourselves are fine.
- **The user prefers autonomous work.** Ask only when visual confirmation or a real decision is needed. When showing results, give file paths to compare images, and explain plainly what changed and why.
- **Heavy computation runs on the Windows NVIDIA machine.** The user's words: "don't run too much on this computer, we want to switch to the windows computer". The Mac (M1, 16 GB) took 8–10 minutes per generation and its disk was 99% full.

## 3. Where things are

**In the repository (`dune-hybrid`, branch `hd-studio` on `github.com/SPD13/dune-hybrid`):**

| Path | What |
|---|---|
| `crates/hdpack` | `dune-hd`: `build` (packs, now with `--art DIR` for finished art), `extract`, `heads` (head frames and layouts), **`inventory`** (every asset as review units for the studio, in true colours) |
| `crates/gfx`, `crates/runner` | Game data decoding; `dune-run`: headless runner (`--rooms` captures every room, `--gfx-trace` records palettes, `--hd 4 --hd-shots --hd-pack` renders HD screenshots through the real compositor) |
| `recipes/` | **The recipe bundle** (v0.1): `bundle.json`, `models.json`, `assets.json` (classification and palettes), **`defaults.json`** (class defaults, hints per sheet, batch rules, time estimates), `heads.json`, **`cast.json`** (recurring characters such as the Atreides guard: one description, one seed), `scenes.json`, `rooms.json`, `interface.json` |
| `tools/portraits/` | Generation CLI. **`hdgen.py`** (batch generation of studio items, any class); `generate.py` (talking heads by recipe, with eye recolour and `--publish`); `recolour.py` (Paul's eye stages); `device.py` (GPU choice and memory sizing); `fetch_models.py`; `requirements-ml.txt`; `requirements.txt` (exact versions tested on the Mac) |
| `tools/studio/` | **Dune HD Studio prototype** (Python, standard library only): `server.py`, `jobs.py`, `system.py`, `index.html`, **`review_todo.py`** (summarises the annotations), `README.md` |
| `docs/` | `hd-studio-plan.md` (plan, phases, pilot results), `generative-portraits.md`, this handoff |

**Outside the repository** (the Mac's `Cryogenic-Web/` folder, to copy to Windows as needed):

| Path | What |
|---|---|
| `DUNE.DAT`, `DNCDPRG.EXE` and the game folder | The user's game files |
| `portrait-proto/studio/` | **The studio's work folder (51 MB): copy it.** `review/items.json` (inventory), `review/review.json` (**the user's annotations**), `inventory/src/` (originals), `versions/` (generated versions), `rooms/` (the 27 room captures), `heads/`, `gen/` (render caches), `traces/intro-palace.jsonl` (palette trace), `studio.json` (Mac paths: delete or edit on Windows) |
| `portrait-proto/` (the rest) | Pilot scripts and outputs, now superseded by `hdgen.py` (`scene_pilot.py`, `room_pilot.py`, `ui_pilot.py`, `make_items.py`, `seed_versions.py`), plus the pilot images. Useful only as history. |
| `dune-hd-pack-pilot.zip` | Pilot pack: the generated throne room, `DN20`, icons 000 and 009. Never share it. |

## 4. How the game's art works (facts we rely on)

- **The DAT:** 1,947 sprites in 128 sheets, plus 27 rooms the game assembles (captured by `dune-run --rooms`), plus 19 talking heads. Some art is drawn by code and stays outside packs: the globe, the map, sky gradients, transitions, the cursor.
- **Talking heads** (`LETO.HSQ` …): faces assembled from part groups. The frame table lists every face the game can show (206 frames); mouths come from the lip-sync table. Paul's 9 eye stages are separate eye-strip sprites that *replace* the strip, so they are frames (`fNN`), not groups drawn over frame 0. The Fremen sheets (`FRM1`–`3`) list about 50 complete faces built from shared hair, beard and face parts.
- **Palettes:**
  - many sheets carry no colours: the desert views `DN*`, `DS*`, `DV*`, `DH*`, `DF*`, `DP*` are **grey** in the DAT and coloured at run time by the time-of-day palettes in **`SKYDN.HSQ`** (entries 8–40: 33 palettes for indices 73–239);
  - the village and greenhouse views (`VIL*`, `VG*`) use `SKYDN:0+SKY:0`;
  - the interface (`ICONES`…) and room sprites use the game palette, taken from a room capture.

  `dune-hd inventory` resolves palettes in this order: a palette trace, then the sheet's own chunks, then the rule in `recipes/assets.json`, then a guess. Items whose colours aren't certain get a ⚠ (146, mostly the intro subtitles).
- **Pack art** is stored per HD pixel as two palette entries plus a weight. That keeps fades and day/night working. **Scenes, rooms and the interface fit this format** once the generated picture is **colour-anchored**: its lightness detail is laid over the original's broad colours, because models drift (pink sand comes out tan). **Talking heads and character sprites need a new true-colour format** (pack v4, see the plan), not built yet.

## 5. The generation pipeline (what works)

SDXL img2img, model `SG161222/RealVisXL_V5.0` with the `xinsir/controlnet-tile-sdxl-1.0` tile ControlNet and the `madebyollin/sdxl-vae-fp16-fix` VAE. The guide is the pixel art enlarged and blurred. Fixed seed 1992, 30 steps, guidance 6.

| Class | Method | Settings that worked |
|---|---|---|
| Talking heads | The frame padded to a square, 1024 px. Shape variants (mouths, a tired overlay) are a **rerender** with the same seed, kept only where the frame differs. Colour variants (eye stages) are **recoloured**, with no generation. | strength 0.85, cond 0.6, blur 4 (Paul 3), per-character description and negatives (`recipes/heads.json`) |
| Scenes | Whole picture at 4×, colour-anchored, palette form | 0.85, 0.6, blur 4, anchor 8 |
| Rooms | Whole room at 4×, colour-anchored. **Parts** (people and objects painted into the room) are redone in a 12× close-up and pasted back inside their outline. | 0.85, 0.6, blur 3, negatives against depth of field; part: 0.85, 0.5, blur 1.5 |
| Characters in rooms | A crop around the character enlarged 12× over the HD room, kept inside the silhouette | 0.9, cond 0.45, blur 1.5 (lower strength keeps them blurred, read as out of focus) |
| Interface ornaments | Enlarged so the model sees about 1024 px, then 4×, colour-anchored, original silhouette | 0.75, cond 0.7, blur 2 |

**What was rejected** (don't retry):
- low strength (≤ 0.65): it only repaints the pixel art;
- a sharp guide on beards: grainy;
- inpainting mouths: they grew stubble;
- warping the HD face along the pixel art's motion: unnatural wrinkles;
- placing eye colours by pixel position: magenta lids;
- a "young" Paul;
- guards described only in the room prompt: they came out robed, which is why parts exist.

**Decided by the user:**
- Leto clean-shaven (approved).
- Paul older and angular; keep his generated skin colour (approved).
- **Separate models per class where they give better results.**
- Their first annotation: Leto's vest emblem must be the Atreides falcon head (see §8).

**Reproducibility:** on the same machine a recipe gives bit-identical images. Paul was rebuilt identically, including through the studio. Across GPUs (Apple MPS against CUDA) results will differ slightly: measuring that is phase 6 of the plan, on the Windows machine.

## 6. Dune HD Studio

```
python tools/studio/server.py WORK --dat path\to\DUNE.DAT --dune-hd target\release\dune-hd.exe [--env DIR] [--port 8765]
```

It opens `http://127.0.0.1:8765/` and listens only on 127.0.0.1. Options are remembered in `WORK/studio.json`.

**Setup** (header button):
- shows the game file, `dune-hd`, the graphics card (from `nvidia-smi`), the generation environment and the models, with free disk space;
- **Install** creates a Python environment in `%LOCALAPPDATA%\DuneHD\env` and installs PyTorch built for the machine: CUDA wheels from `https://download.pytorch.org/whl/cu128` on NVIDIA; only the driver is needed. Then it installs `tools/portraits/requirements-ml.txt`. It uses `uv` if installed, else `venv` and `pip`;
- **Download** fetches only the needed model files (about 9 GB) into the Hugging Face cache;
- `--env DIR` uses an existing environment, which the studio never deletes.

**GPU use** (`tools/portraits/device.py`):
- **CUDA, fp16:** 12 GB of VRAM or more runs with no memory saving; 8–12 GB slices attention; under 8 GB uses CPU offload;
- **Apple GPU (MPS):** fp16 with slicing;
- **CPU:** fp32;
- `DUNE_HD_DEVICE=cuda|mps|cpu` forces a device.

**Review UI:**
- **Sidebar:** classes, then sheets, with approved/total counts; search; filters (not reviewed, needs work, approved, rejected, with generated versions, with boxes, colours not certain).
- **Viewer:** Viewer/Grid (G); Left and Right version pickers; side by side, slider, flip (Space), detail map (D: where a version adds fine detail, cyan to yellow); zoom with the wheel; F, 1 and 2 fit, show 1:1 and 2:1.
- **Item panel:** class, sheet, size, colour source, status, **Generate a new version**, preferred version, notes, boxes.
- **Jobs** (header button): live progress bars and logs, with cancel.
- **Batch** (header button): pick classes, or "the items shown by the current filter"; it shows a count and time estimate (`hdgen.py --plan --json`), then starts one job.

**Annotations** are saved automatically about 0.7 s after each change to **`WORK/review/review.json`**:
```json
{"assets": {"head/LETO/f00": {"status": "new|needs-work|approved|rejected", "preferred": "<version id>", "notes": "…",
  "boxes": [{"kind": "part|problem|keep|note", "text": "…", "x": 0.18, "y": 0.73, "w": 0.15, "h": 0.06,
             "px": [28, 110, 50, 119], "version": "<version the box was drawn on>"}], "updated": "…"}}}
```
- **`x, y, w, h`** are fractions of the picture; **`px`** is the box in the **original's pixels** (`x0, y0, x1, y1`). Use `px` for recipes (room parts take low-res boxes).
- **Kinds:**
  - **part:** something painted into the picture to describe and redo in close-up, with the text as its prompt;
  - **problem:** something wrong to fix;
  - **keep:** keep the original or classic art here (lettering, functional interface);
  - **note:** a comment.
- **Status and preferred version:** *approved* plus a preferred version means the result can go into the recipe as approved and into packs.

**Items and versions:**
- **`WORK/review/items.json`** (from `dune-hd inventory`). Ids are `head/NAME/fNN`, `room/<key>` and `sprite/RES-PPP`. Each item has `class_key`, `group` (sheet or room file), `kind`, `size`, `palette {source, ok}`, `art_name` (the name `dune-hd build --art` expects), `groups` (head frames), `original`.
- **Versions** live in `WORK/versions/<id with / as __>/NAME.png`, with an optional `NAME.json` (`label`, `settings`, `default`). The server attaches them; the page reloads when a job finishes.
- **Generate a new version runs the item's current recipe.** It does **not** read the annotations; with an unchanged recipe it produces nothing new. Annotations are acted on through the loop below.
- **Try settings** (under Generate; not for recipe heads): the user changes strength, control, guide blur, detail-pass strength, seed or prompt and generates a *try*, a new version labelled with what changed (`hdgen.py batch --override JSON --label …`); the recipe is untouched. When the user approves a try (preferred + approved), fold the settings from its `versions/…/NAME.json` into `recipes/`. Room parts and emblems still apply on top of a try. Rough guide on the palace: 0.95 / control 0.4 / blur 5 invents more (ornate throne, drapes) and keeps the layout; 1.0 / 0.3 / 6 starts to move things (doors became cabinets).

## 7. The loop: acting on the user's annotations

1. **Read:**
   ```
   python tools/studio/review_todo.py WORK
   ```
   Add `--json` for machine-readable output. It lists items marked needs-work or rejected, or with notes or boxes, each box with its kind, `px` and text.
2. **Look:** open the item's original (`WORK/inventory/src/...`) and its preferred version, and crop around each box.
3. **Turn annotations into recipes** in `recipes/`, the shipped bundle:
   - **Rooms:** a part box becomes an entry in `rooms.json` `rooms[<key>].parts`: `box` from `px`, `prompt` from the text, `select` (`cool` picks cool-coloured pixels against a warm room), optional `negative_extra`. Problems usually change the room's prompt or settings.
     - **The look is layered** (`recipes/style.json`, the user's hierarchy, 2026-10-14): **game** (David Lynch's Dune, 1984: retro-futuristic, baroque industrial, not oriental) → **location** (matched on the item's group: `PALACE` is a fortress, `SIET` the Fremen sietch, `HARK` the Harkonnen stronghold) → the **item's own prompt**, which only describes what is there. Each layer is encoded as its own 77-token block (`Pipe.embed`), so long prompts are no longer cut. Change the look at the highest layer that applies, not per room. Parts that should stay plain (a patch of floor) or carry their own look (the guard) set `"style": false`.
     - A person who recurs (guards…) names a **`character`** from `cast.json` instead of carrying its own prompt, so every appearance is the same person. The palace guard is `atreides-guard`: Lynch-film look, one face for all guards (user's choice, 2026-10-14).
     - The same sprite drawn twice (the game mirrors the palace guards) uses **`copy_of`** (+ `flip`): the first part's HD result is reused, boxes of equal size.
     - Per part: `strength`, `cond`, `soften`, `zoom`, `seed`, `margin` (crop height ÷ box height; 1.1 keeps a person large enough in the close-up), `close` (fills a face or hands inside a `cool` outline), `darken` (guide only: e.g. a grey rifle the model would read as a third leg).
     - Debug a part on its own before a full run: on this machine a part takes about 10 s, a room about 30–50 s.
   - **Heads:** descriptions, negatives and settings are in `heads.json`. **Parts on heads are not supported yet.** Leto's emblem needs a close-up part redo like rooms have; add it to `generate.py`, reusing `hdgen.refine`.
   - **Scenes:** `scenes.json`, keyed by sheet, e.g. `DN20`. **Interface:** `interface.json`, keyed by `RES-PPP`.
   - **Without a recipe:** an item uses `defaults.json` (class prompt with a `{hint}` per sheet). Add an own entry when it needs specific words.
   - Mark an approved entry's `status` as `approved <date>`. Keep `bundle.json`'s version in step.
4. **Generate:** either
   ```
   python tools/portraits/hdgen.py batch WORK --ids ids.json
   ```
   or `--classes …`; batch versions are keyed by settings, so only changed items are redone. Or, for recipe heads:
   ```
   python tools/portraits/generate.py NAME --heads WORK/heads --out WORK/gen/NAME --only fNN --publish WORK
   ```
   Or tell the user to press Generate or Batch in the studio. Run generation with the environment's Python (`%LOCALAPPDATA%\DuneHD\env\Scripts\python.exe`) and `HF_HUB_OFFLINE=1` once the models are downloaded.
5. **Report back:** which items changed, what was changed in the recipes, and where to look in the studio (the new versions appear there). **Record each box acted on:**
   ```
   python tools/studio/resolutions.py WORK ITEM_ID --boxes 1,2|all --version VERSION_ID --note "what was done"
   ```
   It writes `review/resolutions.json`, never `review.json` (the open page would overwrite it). The box then shows as *to check* (dashed, "?") until the user confirms or reopens it in the studio; resolved boxes show only on the version they were drawn on, and `review_todo.py` leaves confirmed ones out.
6. **Packs:** the studio's **Pack** button (or `python tools/studio/export_pack.py WORK --out DIR`, then `dune-hd build --art DIR`) exports the chosen versions as finished art. That's the approved items, or everything generated except rejected ones.
   - The export writes 4× pictures named by `art_name`, with a `.json` sidecar `{"dac": …}` when the sprite's colours came from another palette than its sheet's (saved by `dune-hd inventory` in `WORK/inventory/palettes`).
   - See `--art` in `crates/hdpack/src/main.rs`.
   - **Characters and other sprites in true colour:** add `"true_colour": true` to the sidecar (pack version 4).
   - **Talking heads:** `NAME-fNN.png` for frame NN and `NAME-vGG.png` for group GG drawn over frame 0, at the picture's aspect (the studio's 1024 px portraits are resized to 4×). They become head plates in true colour, per frame and per group.
   - While a character talks, the game assembles frames from groups (face + mouth + eyes), so **every group used while talking needs a `vGG`**. A group without art shows the pixel-art upscale over the generated face.
   - `dune-hd inventory` lists them as `head/NAME/vGG` items (kind `head-group`, 810 in all). `generate.py` makes them as rerenders of frame 0, all of them when run without `--only`; `hdgen.py batch` routes them to it.
   - **They need the head's recipe in `heads.json`:** only LETO, PAUL and STIL have one. The other heads need a recipe before their groups can be generated.

## 8. State and open items (2026-10-14)

**Done:**
- Phase 0 pilots for every class.
- `--art` packs, verified in the game: the throne room v2 with redone guards, and the bottom panel.
- `dune-hd inventory`: 1,254 units.
- Recipe bundle v0.1.
- The studio: review, setup, jobs, generation, batch.
- `hdgen.py` batch: a default batch is 294 items, about 1.4 days on the M1 and an estimated 2–3 hours on an RTX-class GPU.

**Not verified yet:**
- **`hdgen.py`'s picture, sprite and portrait methods end to end.** A 3-item test batch (`room/0764b06e480e7c25`, `head/FRM1/f00`, `sprite/ICONES-003`) was cancelled on the Mac before finishing. Run it first on Windows:
  ```
  hdgen.py batch WORK --ids <file with those 3 ids>
  ```
  Then check the three versions in the studio.
- **The CUDA path:** the PyTorch cu128 install, `device.py` on NVIDIA, and Windows paths throughout.

**Open, by priority:**
1. **Windows setup** (§9), then the 3-item test, then a default **Batch** for the user to review.
2. **The user's annotation:** `head/LETO/f00` part box `[28, 110, 50, 119]`: "The red symbol on his vest should be the falcon head profile, looking left, logo of the atreides". It needs head parts (above).
3. **Optional (offered, not decided):** let **Generate** apply obvious annotations directly. Part boxes would be redone in close-up with their text; notes would be appended to the prompt.
4. **Engine, phase 1:**
   - **Done (2026-10-15, CPU renderer `dune-run` and the web app's GPU path):**
     - true-colour sprite art following the palette;
     - talking-head plates hooked on `draw_talking_head_at_si` (1000:9D2D), keyed by the drawn parts (`gfx::head`). A draw is covered by the longest consecutive runs of parts with plates, so face + mouth + eyes combinations work without a plate per combination.
     - Verified on Leto's approved portrait in conversation (`dune-run`). The web shader (`hd.frag`) is checked against the CPU renderer's numbers in headless Chrome, but not yet in the running web app.
   - **Still to do:**
     - room plates in true colour;
     - checking a real palette fade on true-colour art;
     - the head path at 1000:9E75 (`loc_09d94`, another origin), not hooked.
5. **Measure** Mac (MPS) against CUDA differences on Paul and Leto, and set QA tolerances (phase 6).
6. **Palette coverage:** a longer scripted play-through with `--gfx-trace`, to settle the ⚠ colours.
7. **Pinned model revisions and licence checks** before any public release (`recipes/models.json`).
8. The plan's later phases: a Rust studio executable with `uv`-bootstrapped Python, one-click mode, distribution.

## 9. Windows setup checklist

1. **Clone the repo:** `git clone --branch hd-studio https://github.com/SPD13/dune-hybrid`. The work is on the `hd-studio` branch, pushed 2026-10-14; it isn't merged into `main` yet.
2. **Install:**
   - Rust (`rustup`), then `cargo build --release`, which gives `target\release\dune-hd.exe` and `dune-run.exe`;
   - **Python 3.12** (python.org; wider PyTorch wheel coverage than the 3.14 used on the Mac);
   - a current **NVIDIA driver** (`nvidia-smi` must work);
   - `uv` (optional: faster installs).
3. **Copy from the Mac:** `DUNE.DAT` and the game folder; the work folder `portrait-proto\studio` (delete its `studio.json`, which has Mac paths). Or rebuild the work folder:
   ```
   dune-run --dir <game> --seconds 90 --events "61.5:key:1:1,61.6:key:1:0" --out run --rooms WORK\rooms --gfx-trace WORK\traces\intro.jsonl
   dune-hd inventory --dat DUNE.DAT --work WORK --recipes recipes --rooms WORK\rooms --trace WORK\traces\intro.jsonl
   ```
   The user's annotations exist only in the copied `review\review.json`.
4. **Start the studio:**
   ```
   python tools\studio\server.py WORK --dat <DUNE.DAT> --dune-hd target\release\dune-hd.exe
   ```
   Then **Setup → Install** and **Download**. Check that the environment line says "uses the NVIDIA GPU: …".
5. **Test:** the 3-item test batch, then a default Batch, after telling the user the estimate.

## 10. Gotchas

- **diffusers 0.41:**
  - `pipe.vae.enable_tiling()`, not `pipe.enable_vae_tiling()`;
  - `from_pipe(...)` needs `torch_dtype=` or it casts to fp32;
  - CLIP reads only 77 tokens, so prompt prefixes (talking or tired prompts) go **first**.
- **Head frames in the studio are frame-sized:** portraits are generated on a 1024 square (the frame padded at the bottom, centred) and **cropped back** (`generate.py --publish`, `hdgen.portrait`). Other items keep the original's aspect at 4×. Boxes are stored as fractions, so a version must have its original's aspect.
- **`git mv -k` silently skips untracked files.** Moving `characters.json` to `recipes/heads.json` once did nothing that way; check after moving. `crates/gfx/examples/sheets.rs` is tracked: don't delete that folder.
- **The studio remembers the status filter in the browser,** but not the search. If items seem missing, check the "Showing N of 1254 · clear filters" line.
- **Long runs:** on the Mac they needed `caffeinate -i` (a sleeping machine stalled them); on Windows, keep the machine awake during batches. Runs sometimes died silently right after loading under memory pressure (16 GB Mac); rerunning works, since batches resume.
- **Wrong cargo:** `/usr/local/bin/cargo` on the Mac was an x86 binary; use rustup's.
