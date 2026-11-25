# 3D talking heads: the Tripo + FLAME pipeline

**For:** a person or a Claude instance building, fixing or extending the 3D talking heads. Written 2026-11-24, updated 2026-11-25 (eyelids, lash strips, rounded mouth corners, per-head build settings, lip-sync to the game's voice). To build a new head, follow the [playbook](#1a-playbook-a-new-head-step-by-step). This is an experiment next to the 2D studio work described in [generative-portraits.md](generative-portraits.md) and [hd-studio-plan.md](hd-studio-plan.md).

**Preview page:** [http://127.0.0.1:8790/tripo/preview.html](http://127.0.0.1:8790/tripo/preview.html) lists every character with a talking head in the game; the ones with a green **3D** badge open their talking page, the 1992 head on the left and the 3D head talking in sync on the right. Serve it first (see [§8](#8-files-environment-and-rules)).

## 1. What it makes

A **3D talking head** for one game character: a textured 3D bust with a rigged mouth (open, wide, narrow, pucker, upper lip raised), real eyeballs that look at the viewer, eyelids that blink, expressions around the eyes and a small head sway. It talks either on a made-up syllable loop built from the game's own mouth groups, or in sync with the character's real voice lines from the CD version.

The pipeline, one character at a time:

| # | Stage | Tool | Output | Cost |
|---|---|---|---|---|
| 1 | HD portrait from the game's frame 0 | Tripo image-to-image (or the 2D studio) | `img2img/<CODE>-tripo.png` | 5 credits |
| 2 | 3D bust from the portrait | Tripo image-to-model | `<name>/model.glb` | 60 credits |
| 3 | Face landmarks, scale, FLAME fit | `view_tripo.py`, `prescale.py`, `prep_tripo.py`, `fit.py` | `flame-fit/<dir>/fit.pt` | ~1 min on the GPU |
| 4 | Mouth rig | `build.py` | `<name>/<name>-flame.glb` (~140 MB) | ~2 min on the GPU |
| 5 | Talking page | `make_page.py` + a character config | `index-<name>.html` | — |
| 6 | Lip-sync to the voice lines | `voice/extract_voc.py`, `voice/lipsync.py` (Rhubarb Lip Sync) | `voice/<name>/<clip>.wav`, `.cues.json` | ~3 s per line |

1 Tripo credit = $0.01. A new character costs about 65 credits.

## 1a. Playbook: a new head, step by step

Commands run from `Cryogenic-Web/portrait-proto/studio/tripo/` with `PY=%LOCALAPPDATA%\DuneHD\env\Scripts\python.exe` and `F=../../flame/model` (from `flame-fit/`: `../../../flame/model`). `<CODE>` is the head's code (`LETO`), `<name>` its folder (`leto-photo`), `<dir>` its fit folder (`flame-fit/letop`).

**Before you start**
1. Ask the user before spending Tripo credits (about 65 for a new head).
2. Serve the pages (§8) and open the preview.

**A. The bust (§2–§3)**
1. HD portrait: `tripo_img.py ../heads/<CODE>-f00-x6.png img2img/<CODE>-tripo.png "PROMPT"`. Keep the frame's aspect. Check that the portrait keeps the framing and gaze, has no pixel blocks left (ask for "real hair strands") and doesn't drift towards a real actor (don't name the film for a living character).
2. Bust: `tripo.py img2img/<CODE>-tripo.png <name>`.

**B. The fit (§4)**
1. Landmarks: render with `view_tripo.py`, write `<dir>/picks-raw.json`. Check each eye corner on a close render.
2. `prescale.py`, `prep_tripo.py`, `fit.py`. Look at `fit_front.png` / `fit_side.png`; the median error should be about 0.13 mm.

**C. The eyes (§5a, §5b)**
1. Measure each eye opening on a close render (3 cm tall) and write `<dir>/eyes.json`. Corners where the white ends; `l` is the eye on the viewer's left. One `radius_abs` for both eyes (0.0255 so far), `iris` = asin(iris radius / 0.0255), `pupil` = 0.4 × `iris`, hand-set `iris_rgb` (and `iris_inner_rgb` for a two-tone iris).
2. The eyelids and lash strips need no measuring: the build finds them from the eye openings.

**D. The rig (§5)**
1. Per-head settings that differ from the defaults go in `<dir>/build.json` (for example `{"CREASE_MM": 1.5}`), never only on the command line: a rebuild without them silently brings a fixed fault back.
2. `cd flame-fit && $PY build.py $F/flame2023_Open.pkl $F/FLAME_masks.pkl ../<name>/model-n.glb <dir> ../<name>/<name>-flame.glb`.
3. Read the build's report lines:
   - `build settings from …`: your `build.json` was read;
   - `lip crease vs texture line`: values beyond about ±2.5 mm mean the cut may sit in the other lip;
   - `round corners: … the opening closes at …`: 6–12 mm inside the corner landmarks so far;
   - `eye … DIAG`: the two eyes' widths close, the ball standing out by a similar amount;
   - `eye … LID`: columns with no skin under the lower edge should be few;
   - `eye … lash strip … pinned at …`: both ends of each strip pinned.

**E. The page (§6)**
1. Write `<name>.json` (copy `leto-photo.json`): code, glb, landmarks, the mouth groups' shapes, blink groups, `expr`, `teeth`.
2. `$PY make_page.py ../<name>.json ../index-<name>.html`, then add the page to `PAGES` in `preview.html`.

**F. The voice (§6a)**
1. `cd voice && python extract_voc.py ../../../../DUNE.DAT <folder> <short>` (Leto: folder `PA`, short `leto`).
2. `python lipsync.py ../../../../DUNE.DAT <folder> <short> ../../../tools/rhubarb-1.14.0/rhubarb.exe <clips…>`. Check that each printed line of text is what the clip says.
3. List the lines in the config's `voice` and regenerate the page.

**G. Check (at a close view, with `document.getElementById('orbit').checked = true` so the head holds still)**
1. **Mouth:** `window.force = {open: 0.62, wide: 0.4, round: 0, upper: 0.3}` (group 33), then wide open `{open: 1, wide: 0.6, upper: 0.5}`, rounded `{open: 0.45, wide: -0.6, round: 0.8, upper: 0.2}`, and closed. No peaks along a lip edge, no black wedges at the corners, nothing hanging in front of the teeth.
2. **Eyes:** open, `blink: 0.5` and `blink: 1`: no white round a closed eye, the lashes come down with the lid.
3. **Voice:** play each line, listen and watch: closed lips on M/B/P, no movement in the pauses.
4. **Orbit:** the eyes follow, then stop.
5. If something is wrong, see [§6b](#6b-diagnosing-the-mouth) and §7. When a fix needs a setting, put it in `build.json` and say so in this document.

**H. Wrap up**
1. Back up the previous model before a risky rebuild (`backup-…/` folders).
2. Nothing game-derived goes into dune-hybrid. Add the head's scripts, configs, measurements and models to the whitelist of the private talking-heads repo only when the user asks to commit.

## 2. Stage 1: the HD portrait

Tripo's **image-to-image** endpoint repaints the pixel art at 2048 px while keeping its composition. It beat our SDXL studio pass at staying faithful (no invented epaulettes or eye colours) and needs no tuning.

- Input: the game's `f00` head frame enlarged 6× with nearest-neighbour (`heads/<CODE>-f00.png`).
- Script: `tripo/tripo_img.py IMAGE OUT.png "PROMPT" [--model seedream_v5] [--size 2048x2048]` (`POST /v3/generation/image-to-image`, default model `seedream_v5`).
- Prompt: "Turn this 1992 pixel-art game portrait into a high-detail … HD portrait. Keep exactly the same framing, pose, head angle, gaze, background and colours." + the character's description from `recipes/heads.json` + the style:
  - **Photographic:** "photorealistic … film still from David Lynch's Dune (1984), dim amber light". Used for the current Leto and Jessica heads.
  - **Painted:** "semi-realistic digital painting with visible soft brushwork …, slightly stylized forms like 1980s science-fiction book cover illustration and the original game art … not a photograph, and not a cartoon". Sits between the game art and the photo.
- Keep the size's aspect to the frame's (Jessica's 173×150 frame → `2208x1920`).

Stage 2 could also start from an approved 2D studio portrait (`versions/head__<CODE>__f00/…png`), as the first Leto and Jessica heads did; those heads are retired, and the work continues on the photographic heads.

## 3. Stage 2: the 3D bust

`tripo/tripo.py IMAGE OUTDIR` uploads the image (`POST /v3/files`), runs `POST /v3/generation/image-to-model` with `model v3.1-20260211`, `texture_quality detailed`, `geometry_quality detailed`, `pbr`, `orientation align_image`, polls the task and downloads `model.glb` (~60 MB, ~1M vertices) and a preview. The API key is read from `TRIPO_API_KEY`.

Tripo has no pose parameter: what the picture shows is what you get. For bodies to be rigged later (guards, Leto's full figure), give it an A-pose with the arms away from the body; hands touching the body fuse with it and cannot be rigged cleanly.

## 4. Stage 3: landmarks and the FLAME fit

[FLAME 2023 Open](https://flame.is.tue.mpg.de/) is a learned 3D model of human faces (5,023 vertices; 300 shape and 100 expression parameters, a jaw joint). We fit it to the Tripo bust so its expression space can move Tripo's surface.

1. **Pick landmarks by eye.** `flame-fit/view_tripo.py MODEL.glb OUT.png front [y0 y1 z0 z1] [res]` renders the bust's points with a coordinate grid (front view: screen right = −z, up = +y). Zoom on the mouth and each eye and write `<dir>/picks-raw.json`: `[y, z]` for both mouth corners, upper/lower lip middles, the lip seam middle, the inner and outer corner of each eye, and the chin.
   - Check the eye corners on a close render: a render cut at its edge once made Jessica's eyes look half as wide as they are, and the blink then closed only half of each eye.
2. **Scale.** `prescale.py MODEL.glb PICKS.json OUT.glb OUT_PICKS.json` scales the bust so its mouth is Leto's width and moves its lips to Leto's place: every later constant was tuned on Leto's first bust.
3. **Prepare.** `prep_tripo.py MODEL-n.glb DIR PICKS.json` turns the picks into 3D landmarks (the front-most surface point at each pick) and samples the head's points, normals and colours (`tripo_head.npz`, `tripo_landmarks.json`).
4. **Fit.** `fit.py FLAME.pkl MASKS.pkl DIR` aligns FLAME on the landmarks (Umeyama), then fits pose, shape, expression and finally per-vertex offsets to the surface, robust point-to-plane on the GPU. Check `DIR/fit_front.png` and `fit_side.png`; a good fit reports a median error around 0.13 mm.

## 5. Stage 4: the mouth rig (`build.py`)

`build.py FLAME.pkl MASKS.pkl TRIPO-n.glb DIR OUT.glb`

1. **Lip shapes.** Five FLAME expression + jaw solutions, each regularised so the eyes, nose and scalp stay still: `open` (jaw 0.24 rad), `wide`, `narrow`, `pucker`, `upper`.
2. **Binding.** Every head vertex of the Tripo mesh is bound to the closest point on the fitted FLAME surface; lip vertices are bound on their own side (upper lip off the jaw, lower lip on it). The motion fades out where Tripo's surface leaves FLAME's (hair, ears, collar).
3. **The lip line.** Found in the texture (the dark line between the lips), then snapped to the geometric crease within ±`CREASE_MM` (default 3 mm) and smoothed.

   **Per-head settings** such as `CREASE_MM` and `UPPER_STRIP` go in `DIR/build.json` (Leto photo: `{"CREASE_MM": 1.5}`), so every rebuild uses them. An environment variable still overrides. Passing them only on the command line once lost Leto's value in a later rebuild, and his lower lip pinched again.
4. **Split, not cut.** Every triangle the lip line crosses is split exactly on the line; the new points get two copies, one riding each lip, so both lip edges follow a smooth curve.
5. **Smooth motion.** Each lip's motion is averaged over that lip only (never across the slit); past the corners the two lips blend back into one surface (`CORNER_MM`, default 2 mm inside to 6 mm past each corner), so the corners taper closed. Away from the mouth the motion is smoothed too, which removes ragged bands along the jaw.
5a. **Rounded corners** (`ROUND_CORNERS`, default on). With the mouth open, the gap between the lips follows an ellipse in the middle, but near each corner the face model leaves a thin sliver of a gap running on to the corner landmark. The skin there folds and shows its dark back: pointed black wedges biting past the corners. Per side, the build fits the ellipse to the middle of the `open` gap and draws the lips together wherever the gap runs past it, for every mouth shape, so the opening ends round on its own curve. The inner-lip strips fade out at the new ends. The build prints where the opening now closes against the corner landmarks (Leto 6.5 mm inside, Jessica 8 mm, Paul 9–12 mm). `ROUND_W` scales the ellipse's half-width (above 1: wider corners).
6. **Inner lips.** A two-row strip curls from each lip edge into the mouth so open lips look rounded. The upper strip tucks up steeply (`UPPER_STRIP`, default 0.6× back, 2.2× curl) so it never hangs in front of the teeth.
7. **The inside.** A mouth bag (a dark cup), a tongue, and upper and lower rows of teeth on a dental arch; the lower parts ride the jaw. Side teeth darken towards the back.
8. **Shading.** The lip edges and the skin next to them darken as the mouth opens (a colour morph on `open`), so a closed mouth has no smudge.
9. **Eyeballs and eyelids (optional).** With `DIR/eyes.json` the painted eyes are replaced by real eyeballs that the page can turn ([§5a](#5a-eyeballs-and-gaze)), and each gets an upper eyelid that closes over it ([§5b](#5b-eyelids)).
10. **Output.** A GLB with sparse morph targets on the Tripo mesh and separate meshes for the bag, tongue and teeth, each with an `open` target.

## 5a. Eyeballs and gaze

Tripo paints the eyes onto the face's surface: there is no eyeball to turn, and sliding the texture to move the iris distorts it. `build.py` therefore cuts a socket and puts a real eyeball in each eye.

**1. Measure each eye opening** on a close front render (`view_tripo.py MODEL-n.glb OUT.png front y0 y1 z0 z1 900`, about 3 cm tall around the eye) and write `DIR/eyes.json`, one entry per eye. `l` is the eye at positive z, on the **viewer's left** (the character's right); `r` is on the viewer's right (the character's left). Say which one you mean as seen by the viewer, to avoid moving the wrong eye.

| Key | Meaning |
|---|---|
| `inner`, `outer` | the two corners `[z, y]`, **where the white of the eye ends**, not on the lashes |
| `mid`, `top`, `bottom` | the middle's z, the upper lid's y and the lower lid's y there |
| `radius_abs` | the eyeball's radius in model units: give **both eyes the same value** (Leto, Jessica and Paul photo: 0.0255) |
| `iris`, `pupil` | their angular radius on the ball (radians); pick `iris` = asin(iris radius / `radius_abs`) so both irises match |
| `iris_rgb`, `sclera_rgb` | colours (otherwise sampled from the painted eye; a hand-set value is safer: Leto `[78, 88, 96]`, Paul `[84, 98, 112]`, sclera `[138, 126, 110]`) |
| `iris_inner_rgb` | optional two-tone iris: this colour round the pupil, fading to `iris_rgb` towards the rim (Jessica's hazel: `[118, 80, 38]` inside, `[92, 92, 48]` outside) |
| `behind` | how far behind the lid surface the ball sits (m): negative brings it forward, against the lids (Jessica −0.002 and −0.0035) |
| `shift_z` | moves the ball (and where its pupil shows) along the face, for fine placement; leave unset when the corners are right |
| `margin`, `margin_low` | how far outside the lid lines painted-eye colour is also cut (default 2.5 mm; `margin_low` below the lower lid, 0 where lashes there are mistaken for eye) |
| `lid_gap`, `lid_shade`, `lash`, `lid_cover`, `lid_past`, `lash_h`, `lash_bend` | the eyelid's settings, see [§5b](#5b-eyelids) |

Measure each corner where the **white ends**: lashes, the dark crease at the inner corner and the shadow at the outer one all look like part of the eye at a glance. Jessica's inner corner first sat 1 cm into the crease: the ball centred in that opening sat off towards the nose, and no nudge fixed it until the corner was re-measured. An outer corner past the white shows a sliver of eyeball beyond the lid.

The early lesson: corners measured out on the lashes made one opening 70 % too wide, so the eyeballs came out different sizes and every later nudge just compensated. Measure carefully once, use one radius for both eyes.

**2. The build**, per eye:
- the lids are two parabolas through the corners and the middle points;
- every surface point inside the opening, plus painted-eye colour just outside it (not skin-coloured), is removed, and any small flake around the eye left unattached to the face goes too (no floating lash or lid pieces);
- a sphere of `radius_abs` is centred on the opening's middle, just behind the lid surface, with a painted iris texture (pupil, radial streaks, darker limbal ring, toned-down white);
- the build prints a `DIAG` line per eye: the lid surface, the upper and lower rim depth, the ball's front and how far it stands out past each rim, the opening's width and height, and the ball's position against the opening's middle. Compare the two eyes there: the widths should be close, and the ball should stand out by about the same average amount (Jessica: 5.2 / 5.7 and 3.4 / 7.4, a heavier upper lid on one side); match them with `behind`;
- the GLB gets a node `eyes` (the bust's transform) with `eye_l` / `eye_r` children; each carries `extras.fwd` (the ball's rest axis) and `extras.open` (the opening's middle). The eyelids `lid_l` / `lid_r` sit next to them (§5b).

**3. Gaze on the page** (`index-flame.html`, every frame):
- *neutral*: the direction that puts the pupil in the middle of the opening seen from the page's default camera;
- *look at the viewer*: by default (`window.lookAtCamera = true`) each eye turns from neutral towards the camera, clamped to a human range of about 25° sideways and 12° up or down (`window.EYE_LIMIT = [0.44, 0.21]` radians; the busts are turned ~22° from the viewer, and a tighter limit left the eyes short of the viewer, focused somewhere behind them: the blank stare), so orbiting round the bust the eyes follow and then stop instead of rolling into the corners;
- *focus*: the eyes aim at a point on the line from between them to the camera, `window.EYE_FOCUS` (default 4) times farther than the camera. At the camera's true distance (about 1 m at the head's scale) they converge ~3.8° and look cross-eyed; at 4× the lines of sight converge under 1°, like someone looking at you from across a table;
- *saccades*: small quick glances off the viewer and back (on their own clock, so they continue when talking is paused);
- the eyes follow the head sway; with `lookAtCamera = false`, `window.gaze = [yaw, pitch]` sets them directly. (On a model without eyelid meshes they also tuck back while the skin folds shut.)

**Expression around the eyes.** `build.py` also adds three morph targets solved in FLAME's expression space with the mouth, nose and scalp held still: `brow_down` (brows lowered, the inner ends more and drawn together), `brow_up` and `squint` (lower lids raised). The page keeps a resting level of lowered brows and squint, presses both harder on stressed (wide-open) syllables and lifts the brows briefly at some phrase starts. Strengths per character in `CHAR.json` `expr` (`brow`, `squint`, `emph`, `lift`; Leto 0.4 / 0.35 / 0.4 / 0.35, Jessica 0.25 / 0.25 / 0.3 / 0.45, Paul 0.3 / 0.3 / 0.35 / 0.4). The pupils are 40 % of the iris width: smaller pupils read as a blank stare.

**The spice stages (eyes of the Ibad).** As the game goes on, the spice turns Paul's eyes blue. His head file is the only one with a series of eye stages: 9 of them, each its own eye sprite, each also with the tired face, so 18 frames (`f00`, `f02` … `f16`; odd frames are tired). The game picks the stage from the game time (1000:917A): the time (16 slots a day) shifted right by 6, so one stage every 4 days, capped at 8 from day 33 on. The head frame is twice the stage, plus 1 when the desert fatigue ([F4h]) has reached 10h. No other head changes: Leto, Jessica and the rest keep one set of eyes, and Stilgar's and the Fremen's are blue from the start.

The colours, averaged from Paul's frames by what each pixel is at stage 0 (`[white, iris, pupil]`, RGB):

| Stage | Days | White | Iris | Pupil |
|---|---|---|---|---|
| 0 | 1–4 | 151, 153, 139 (grey) | 74, 67, 54 (brown) | black |
| 1 | 5–8 | 137, 154, 155 (blue-grey) | brown | black |
| 2 | 9–12 | 85, 118, 176 (blue) | brown | black |
| 3 | 13–16 | 78, 117, 185 | brown | black |
| 4 | 17–20 | 78, 117, 185 | 59, 70, 86 (grey-blue) | black |
| 5 | 21–24 | 68, 106, 182 | 42, 73, 181 (blue) | 11, 22, 140 |
| 6 | 25–28 | 74, 117, 189 | 27, 51, 176 | 0, 0, 97 (dark blue) |
| 7 | 29–32 | 65, 115, 201 | 21, 40, 174 | 0, 0, 97 |
| 8 | 33 on | 55, 104, 202 | 11, 22, 152 | 0, 0, 97 |

The page's **eyes** menu shows any stage: `EYE_STAGES` in `index-flame.html` holds the table. The eyeballs' shader recolours each part of the 3D eye by its change from stage 0, telling the white, iris and pupil apart by the angle from the eye's axis (`iris`, `pupil` from `eyes.json`, put in by `make_page.py`): the white and iris as a ratio, the pupil as the colour itself once it turns blue. So a head's own iris colour carries through (Jessica's hazel turns a deeper blue than Paul's grey). With `"eye_frames": true` in the config (Paul), the 2D head shows the game's own frame for the stage. The menu is offered only for heads whose game frames have eye stages (`eye_frames`, Paul so far); every other head keeps its own single set of eyes, stage 0, with no menu. `setEyeStage(k)` sets the stage from the console.

**4. Check** with the mouth closed and `window.force = {open: 0, blink: 0}`: a close view of each eye (white sliver past a corner = corner too far out; gap round the ball = `behind` too far back; missing lid piece = lower lid line too low or `margin_low`), a full blink (`blink: 1`), and an orbit.

## 5b. Eyelids

The first blink folded the painted face: a shader squashed each eye opening onto its lower lid. That fold drags whatever is painted above the eye. Where the brows sit right over the lids (Paul) it either pulled the brows down or, held lower, smeared the lid. `build.py` now gives each eye a real **upper eyelid**: a separate mesh that slides down over the eyeball, while the face itself never moves.

**The shape.** The lid is a shell on a sphere around the eyeball's centre, `lid_gap` (default 0.8 mm) in front of the ball, a grid of 40 columns across the eye (from 5 mm before the inner corner to 5 mm past the outer one, `lid_past`) and 26 rows down from its edge. The page moves the edge from **open** (above the upper lid line, hidden behind the skin) to **closed** (on the lower lid). The rows follow above it.

**Where the skin covers it.** For each column the build finds the hole a shell at that depth would show through: it projects the face's triangles around the eye onto the front view and keeps the run of points where no surface lies in front of the shell. Testing vertices was not enough, because a rim vertex's surface only reaches away from the hole: the lid then stopped short and white showed along the rim. The run's top and bottom (`tu`, `tl`, widened by `lid_cover`, default 1.2 mm) bound the band where the lid stands at full depth. Outside it the shell dips 2 mm behind the ball's surface, so:
- open, the whole lid is behind the ball and the skin: nothing shows;
- closed, its edge sits just under the skin that bounds the hole below;
- where the lower lid's skin lies behind the ball (Jessica, whose balls are brought forward), the edge goes down to where the ball itself disappears behind the skin, or white shows under the closed lid;
- past the corners the lid curves are held at the corners' height. The hole there can run above or below the corner line (painted eye cut by colour), so the run is searched around the opening's middle, not from the lid lines.

**The lash strip.** A shell alone closed under the painted lid: the lashes and the lid skin above the eye stayed up, and the closed eye showed them over a bare lid. So the build also cuts the face's band along the top of the eye's hole (`lash_h`, default 4 mm: the lash line, the painted lashes and the lid skin up to about the crease) into its own mesh, `lash_l` / `lash_r`, kept in the face's coordinates and material so the page's head sway moves it with the face. While blinking, the page turns each strip point about the ball's centre by its column's angle (`extras.lash.al`, from the hole's top edge down to the lower lid), and pushes it out to just over the shell near the lash line. The lashes come down with the lid, the closed eye shows the lash line resting on the lower lid, and the shell shows above the strip like lid skin coming out from under the crease.
- The strip starts at the hole's top edge, not at the measured lid line: the painted-eye cut has usually taken the lower part of the lash band already.
- The hole is measured twice, before and after the cut, and where a strip was cut the lid reaches at least the strip's top (a bridge of skin can split the hole there).
- The strip's two ends stay joined to the face: the turn fades to 0 within `lash_bend` (default 5 mm) of the points they share with it, so the strip bends there instead of tearing.
- The shell keeps its own path (`extras.lid.al`) down to just under the skin that bounds the hole, so it also covers the hole past the corners, where the strip barely moves.

The scripts and models from before the strip are kept in `tripo/backup-lids-v1/`.

**The texture.** The lid takes its skin from the band 3 mm above the strip's top (under the brow), stretched over the shell. Anything in that band that is not skin-coloured takes the band's skin colour. `lash` darkens the shell's edge (default 1: none, since the strip carries the real lashes). The whole lid is shaded down for the brow's shadow (`lid_shade`, default 0.88; Leto 1.08, whose band is the dark crease).

**The page.** The lid nodes carry their per-column data in `extras.lid` (`zs`, `yu`, `yl`, `tu`, `tl`, `al`, `H`, …). Every frame `moveLids` follows the head sway like the eyeballs. On a change of blink value, `shapeLid` recomputes the shell's points (the edge turned by `al` × blink about the ball's centre, the radius dipping outside `tl`…`tu`) and its normals, and `shapeLash` turns the strip. The blink is smooth: about 70 ms closing, a short hold, about 130 ms opening, at the same times as the game's own blink frames on the 2D side. `window.LID_REST` (0 by default) lowers the lids a little at rest. With lid meshes the skin no longer folds (the shader's blink stays at 0). Models without them keep the folding blink.

The build prints a `LID` line per eye: the gap, the shell's height, where the open edge hides above the upper lid line, where the closed edge rests against the lower lid, and how many columns found no skin under the lower edge (the lid then rests over that skin).

**Check** with `window.force = {open: 0, blink: 1}` and `0.5` at a close view. To tell a gap from paint, colour the lids flat and hide the balls in the console:
```js
lidNodes.forEach(L => { L.node.material = L.node.material.clone(); L.node.material.map = null; L.node.material.color.set(0x22cc22); L.b = -1; });
eyeNodes.forEach(e => e.node.visible = false);
```
Black left in the opening is a gap in the lid. White still showing is painted eye left on the skin.

## 6. Stage 5: the talking page

`flame-fit/make_page.py CHAR.json OUT.html` makes the page from the Leto template (`index-flame.html`). The config (`tripo/leto-photo.json`, `jess-photo.json`, …):

| Key | Meaning |
|---|---|
| `code`, `name`, `size` | the head's code, its display name, the frame's size in pixels |
| `glb`, `landmarks` | the rigged GLB and the landmarks file (places the eyelids and the camera) |
| `shapes` | per mouth group: `[open, wide, round, upper]`, read by eye from the pixel art |
| `blink`, `half_blink`, `eye_box` | the game's eye groups and where they draw |
| `blink_w`, `blink_h` | the folding blink's width and height × the default (Jessica photo: 0.85, 1.0); only for models without eyelid meshes |
| `eyes_gaze` | the eyeballs' `[yaw, pitch]` when `lookAtCamera` is off |
| `lids`, `lids_top` | for the folding blink only: per eye three `[z, y]` points on the lower lid (inner, middle, outer) to shut on, and where the fold fades out above the eye (`[from, to]` m over the corners, default `[0.0135, 0.0335]`) |
| `yaw` | the whole bust turned (radians), no deformation; unset keeps the portrait's own turn, which matches the sprite |
| `mouth_scale` | the mouth's range of motion, `[open, wide, round, upper]` factors on every shape (the loop, voice lines and `window.force` alike); for a head whose mouth opens too far or too wide (Paul photo: `[0.72, 0.7, 1, 0.8]`); `window.MOUTH_SCALE` tries values live |
| `teeth` | `[r, g, b, env]` tint for the teeth, for a darker portrait (Leto photo: `[0.36, 0.3, 0.24, 0.12]`) |
| `camera_distance`, `target_dy` | framing |
| `intro` | the paragraph under the title |

The page plays a 60-second syllable timeline built from the character's mouth groups (consonants = nearly closed shapes, vowels = open ones), drives the five morph targets and a lip seal that fades as the lips part, closes the eyelid meshes to blink (§5b), and sways the head. A button records 8 s as WebM. `window.force = {open, wide, round, upper, blink}` holds a shape for checking.

To add a character to the preview page, add a line to `PAGES` in `tripo/preview.html`.

**A character who never speaks** (Paul, the player character): his game head has no mouth groups, only the blink, the eyes turning blue and a few brow frames. His config borrows Leto's mouth shapes (keyed 100+, with no images, so the 2D side stays on frame 0 and only blinks); the page shows frame 0 for any group image that is missing.

## 6a. Lip-sync to the game's voice (prototype)

**Where the voice is.** The CD version's speech is inside `DUNE.DAT`, 2,288 Creative Voice files in one folder per talking head: `PA` (Duke Leto, 110 clips), `PB`, … `PO` and `PZ`. A line has two clips, `PAnnnI.VOC` and `PAnnnO.VOC`, for phrase nnn (hex). They share the same mouth track; `O` is the shorter take. `I` is the one used so far. `PB` is Jessica (199 clips; her clip `PB005` begins "I'm your mother, Jessica."). The other folders probably follow the talker numbers (the list of characters in `dune-hybrid-decompile/docs/readable-port-plan.md`: `PC` Thufir Hawat, `PD` Duncan Idaho and so on); confirm each by its text before building a head's lines.

**The clip format.** A clip is a Creative Voice File: a 26-byte header, then blocks (type, 24-bit length, data):
- block 5 (text) holds **the game's own mouth track**: the phrase number, 0, then one mouth-frame byte per step, ending in FFh. The engine plays it alongside the sound (`load_voc_and_lipsync_data`, `lip_sync_frame_task` in the decompile worktree's `hand/voice.rs`). A later step can use it to drive the 2D head exactly as the game does;
- block 1 is the sound: a rate byte (256 − 1,000,000 / rate: 22,222 Hz) and uncompressed 8-bit mono samples.

**The text.** The English phrases are in `PHRASE11.HSQ` (HSQ-compressed; a table of 16-bit offsets, each string ending in FFh; FEh breaks the text into pages; bytes 80h and up stand for names and numbers the game fills in). A clip `<folder>nnn` says phrase *first* + nnn − 1 of that file, *first* being the head's first phrase (the engine makes a phrase number relative to it, from a table it builds at start-up). Leto's first is 0 (`PA001` "I am the Duke Leto Atreides, your father."), Jessica's 49, right after Leto's 49 lines. To find a new head's first, match its clips' lengths to the phrases' text lengths: the true start correlates clearly best (Leto 0.65 at 0, Jessica 0.62 at 49, the next candidates below 0.48). Then check the texts against the clips. `lipsync.py` knows `PA` and `PB`; give another folder as `PC:<first>`.

**The scripts** (`tripo/voice/`):
- `dunedat.py`: reads `DUNE.DAT`'s table of contents and decompresses HSQ (a port of `crates/gfx/src/dat.rs` and `hsq.rs`).
- `extract_voc.py DUNE.DAT FOLDER OUTDIR [CLIPS…]`: every clip of the folder as `OUTDIR/<clip>.wav`, plus `OUTDIR/clips.json` (rate, length, the game's mouth track).
- `lipsync.py DUNE.DAT FOLDER OUTDIR RHUBARB.exe CLIP…`: for each clip, its English text from the phrase file, then [Rhubarb Lip Sync](https://github.com/DanielSWolf/rhubarb-lip-sync) 1.14 on the WAV with that text (`-f json --extendedShapes GHX -d text.txt`). Writes `OUTDIR/<clip>.cues.json`: `{clip, text, duration, cues: [[start s, shape], …]}`. About 3 s per line.
- Rhubarb itself (MIT licence, downloaded from its GitHub releases) is in `Cryogenic-Web/portrait-proto/tools/rhubarb-1.14.0/`, outside both repos.

**The shapes** and what the page makes of them, as `[open, wide, round, upper]` for the 3D mouth:

| Rhubarb | Sounds | 3D target |
|---|---|---|
| A | M, B, P (closed) | `[0, 0, 0, 0]` |
| B | K, S, T, EE (teeth nearly closed) | `[0.12, 0.8, 0, 0.25]` |
| C | EH (open) | `[0.4, 0.5, 0, 0.3]` |
| D | AA (wide open) | `[0.85, 0.4, 0, 0.4]` |
| E | AO, ER (rounded) | `[0.3, -0.3, 0.4, 0.1]` |
| F | OO, W (puckered) | `[0.35, -0.6, 0.8, 0.1]` |
| G | F, V (upper teeth on the lip) | `[0.04, 0.1, 0, 0.5]` |
| H | L (tongue up) | `[0.35, 0.3, 0, 0.3]` |
| X | rest | `[0, 0, 0, 0]` |

The table is `VIS` in `index-flame.html`; tune a shape there and regenerate the pages.

**The page.** The config's `voice` lists the lines (`[{wav, cues, text}]`, paths from `tripo/`). With it the page shows a line menu and **Play line**. While a line plays:
- the cue at the audio's current time plus `VOICE_LEAD` (40 ms: the mouth moves slightly before the sound, as in speech) gives the 3D target, eased in as the syllable loop's shapes are;
- the 2D head shows the mouth group closest to that target (`nearestGroup`);
- the expressions around the eyes follow the mouth as on the loop (stressed open syllables press the brows), and blinks go on;
- the line overrides the made-up syllable loop, which takes over again when the line ends.

**Checking it.** Listen in a real browser window: an automated tab can draw only one or two frames a second, too few to judge motion. Offline, compare the cues with the voice's loudness (20 ms steps): on Leto's first lines the mouth never opens in a silence, and it is shut while the voice is loud on 1–7 % of steps (closed consonants). Jessica's lines: open in a silence on 0–4 % of steps, except the hidden-door line (10 %, the pause in mid-line), shut while loud on 1–7 %.

**The test lines:**
- Leto (`PA001I`–`PA005I`): "I am the Duke Leto Atreides, your father." / "What have you done with Gurney? …" / "Where is Gurney?" / "We don't have enough people working for us. …" / "My son, we must mine the spice as soon as possible …".
- Jessica (`PB004I`, `PB008I`, `PB00AI`, `PB00BI`, `PB00DI`): "Why did you leave me in the desert? Sometimes you frighten me." / "Paul, I tell you these Harkonnens are beasts. They are completely without mercy!" / "I think there is a hidden door on the left. Yes, let's have a look." / "Your friend Duncan is an expert on spice production." / "I wonder what the proximity of so much spice will do to you Paul." Picked for their variety of mouth shapes, and none has a filled-in name.

**Rules.** The clips, WAVs and cues are game-derived: they stay on this machine, in neither repo.

**Not done yet:**
- the other characters' folders and first phrases (find them as above);
- phrases with names filled in (their codes are dropped from the text, so Rhubarb hears a word it wasn't told);
- the French and German voices (Rhubarb's `--recognizer phonetic` mode, without English text);
- the game's own mouth track for the 2D head;
- the lines as the game's own sentences (the page plays one clip at a time).

## 6b. Diagnosing the mouth

What the faults looked like on Leto, and how each was found:
- **Two peaks rising from the lower lip** (group 33): the crease search had gone into the other lip. Leto's bust had been tuned with `CREASE_MM=2` on the command line; later rebuilds dropped it. Fixed with `{"CREASE_MM": 1.5}` in `letop/build.json`, which also smooths the wide-open shape (2 still left two smaller bumps there).
- **Pointed black wedges past the corners:** the lips' gap ran on past the opening's curve, and the skin folded and showed its back. Rounding the blend that closes the corners made the tail longer, not shorter. Smoothing the motion of the folded triangles' corners didn't take either. What worked is §5 step 5a: close the lips where the gap leaves the opening's own ellipse.

The tools:
- **Back faces in magenta, the mouth bag green** (in the page's console): any magenta seen from the front is skin folded over, green past the lips is the bag showing through.
  ```js
  let face; model.traverse(o => { if (o.isMesh && o.morphTargetDictionary?.squint !== undefined) face = o; });
  const m = face.material, old = m.onBeforeCompile;
  m.onBeforeCompile = (sh) => { old(sh); sh.fragmentShader = sh.fragmentShader.replace('vec4(diffuseColor.rgb * 0.3, gl_FragColor.a)', 'vec4(1.0, 0.0, 1.0, 1.0)'); };
  m.customProgramCacheKey = () => 'magenta'; m.needsUpdate = true;
  model.traverse(o => { if (o.name === 'mouth_bag') o.material.color.set(0x00aa00); });
  ```
- **`face.material.side = 0`** (front faces only): if a dark shape disappears, it was a back face; a hole left behind shows where.
- **`DEBUG_SPLIT=1`** (build): prints the gap between the lips at `open` along the mouth and stops. A gap that falls slowly over the last centimetre before a corner is a pointed corner.
- **`DEBUG_CORNERS=1`** (build): counts the triangles around each corner that face away at group 33's shape, by origin (the bust's surface, the split along the lip line, the inner-lip strips), by distance from the corner and height from the lip line.
- A view close to the mouth (in the page's console):
  ```js
  const mth = face.localToWorld(new face.position.constructor(...LIP_SEAM_MID));   // lip_seam_mid from <dir>/tripo_landmarks.json
  const dir = camera.position.clone().sub(controls.target).normalize();
  controls.target.copy(mth); camera.position.copy(mth).add(dir.multiplyScalar(0.45)); controls.update();
  ```
- Check the model really reloaded after a rebuild: `performance.getEntriesByType('resource')` shows the GLB's transfer size (a cached model shows 0).

## 7. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| A pinch or peak on a lip when it opens | the crease search went into the other lip | lower `CREASE_MM` (Leto photo: 1.5; 3 leaves two peaks on the lower lip, 1 a ragged upper edge) |
| A ragged edge on the upper lip | `CREASE_MM` too low: the cut sits in the lower lip | raise it again |
| A flap hanging in front of the upper teeth | the upper inner-lip strip | `UPPER_STRIP` (curl it up more) |
| Spikes or holes in the lips when open | neighbouring vertices bound to different FLAME parts | already handled by the per-lip smoothing; check the landmarks |
| White specks at the corners | teeth seen through holes in the lip | already handled by the closed corners; check the mouth corner picks |
| Teeth too bright | the portrait's light is dimmer than the template's | `teeth` in the config |
| Eyes of different sizes, or the pupils off-centre | eye corners measured on the lashes | re-measure the openings, one `radius_abs` for both (§5a) |
| Floating pieces round an eye | painted lash or lid left in front of the ball | handled by the socket clean-up; widen `margin` |
| A notch in the lower lid | lower lid line too low, or lashes cut as eye | raise `bottom`; `margin_low` 0 |
| White along the rim of a closed eye | the lid dips behind the ball before the skin covers it | raise `lid_cover` |
| White under a closed eye | the ball stands in front of the lower lid's skin | handled (the closed edge goes down to where the ball disappears); check `behind` |
| A straight cut at a corner of the closed lid | the hole runs past the lid's last column | raise `lid_past` |
| A closed lid darker or lighter than the skin | the band above the lid is in shadow or lit | `lid_shade` |
| Dark brackets at the outer corners of a closed eye | the strip's pinned ends twisting, seen edge-on | raise `lash_bend`; at normal distance they hardly show |
| Lashes or dark lid skin left up over a closed lid | the strip is too low | raise `lash_h` |
| A light tick at a corner that never covers | painted eye white left on the skin, not a gap (check with the balls hidden) | widen `margin` |
| The folding blink (no lid meshes) closes only part of the eye | eye corners picked too close together | re-pick on a close render; then `blink_w` |
| The folding blink drags a wide band of skin, or the brows | blink area too tall or wide | `blink_w` below 1, a lower `lids_top`; better, rebuild with eyelids |
| Pointed black wedges at the mouth corners | the lips' gap runs on past the opening's curve; the folded skin shows its back | `ROUND_CORNERS` (on by default); check with the skin's back faces in magenta |
| The mouth corners too narrow or too wide | the fitted opening | `ROUND_W` (e.g. 1.1) |
| A ragged band along the jaw | uneven binding under the chin | handled by the far-field smoothing |
| A fault fixed before comes back after a rebuild | the fix was a setting given only on the command line | put it in `<dir>/build.json`; the build prints the settings it read |
| Light ragged bits at the lower lip's corners (close up only) | the lips meeting where the gap was closed | not fixed; invisible at the page's distance |
| A voice line's mouth looks off | the wrong text for the clip, or a name the game fills in | check `lipsync.py`'s printed text against the clip |

## 8. Files, environment and rules

- **Where things are.** Everything lives **outside the repo**, under `Cryogenic-Web/portrait-proto/studio/tripo/`: the scripts (`tripo.py`, `tripo_img.py`, `tripo_rig.py`, `flame-fit/*.py`, `voice/*.py`), the per-character work (`flame-fit/<dir>/`, with `eyes.json` and `build.json`), the models (`leto-photo/`, `jess-photo/`, `paul-photo/`; `leto-hd/model.glb` stays only for the throne-room scene), the voice clips and cues (`voice/<short>/`) and the pages. Backups from before the eyelids and the lash strips: `backup-before-lids/`, `backup-lids-v1/`. Work continues on the **photographic heads only** (2026-11-25): the earlier studio-portrait heads' pages and configs (`index-leto.html`, `index-jess.html`, `leto.json`, `jess.json`) were removed (they are in the private repo's history), and their rigged models moved to `archive-studio-heads/`. Rhubarb Lip Sync: `Cryogenic-Web/portrait-proto/tools/rhubarb-1.14.0/`. The FLAME files are in `Cryogenic-Web/portrait-proto/flame/model/` (`flame2023_Open.pkl`, `FLAME_masks.pkl`), downloaded by the user from the FLAME site.
- **Environment.** The Windows NVIDIA machine; Python `%LOCALAPPDATA%\DuneHD\env\Scripts\python.exe` (torch with CUDA). The FLAME loader in `flame.py` needs neither chumpy nor scipy. Tripo needs `TRIPO_API_KEY`.
- **Serving the pages.** From `portrait-proto/studio`: `python -m http.server 8790 --bind 127.0.0.1`, then open the [preview page](http://127.0.0.1:8790/tripo/preview.html).
- **Never commit or share game-derived files:** the portraits, busts, GLBs, renders, pages and voice clips all come from the game. They stay on the user's machine. Only this description belongs in the repo. The private repo `SPD13/dune-talking-heads` (git root `tripo/`) holds the pipeline through a whitelist `.gitignore`; the voice clips are not in it.
- **FLAME licence.** FLAME 2023 Open is CC-BY 4.0: anything shared that uses it must credit FLAME (Li et al., "Learning a model of facial shape and expression from 4D scans", SIGGRAPH Asia 2017). The FLAME files themselves are never committed.
- **Credits.** Ask before spending Tripo credits on new characters.

## 9. Related experiments

The same Tripo account built rigged full-body figures placed on the HD rooms: an Atreides guard and rifle (`guard-pose.html`, rigged with Tripo's auto-rig, 25 credits; posed in code with arm IK and hand contacts), the throne room with the guards and Leto (`leto-throne.html`), and the dining hall with depth-based occlusion from Depth Anything V2 (`dining.html`).
