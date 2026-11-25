# 3D talking heads: the Tripo + FLAME pipeline

**For:** a person or a Claude instance building, fixing or extending the 3D talking heads. Written 2026-11-24. This is an experiment next to the 2D studio work described in [generative-portraits.md](generative-portraits.md) and [hd-studio-plan.md](hd-studio-plan.md).

**Preview page:** [http://127.0.0.1:8790/tripo/preview.html](http://127.0.0.1:8790/tripo/preview.html) lists every character with a talking head in the game; the ones with a green **3D** badge open their talking page, the 1992 head on the left and the 3D head talking in sync on the right. Serve it first (see [§8](#8-files-environment-and-rules)).

## 1. What it makes

A **3D talking head** for one game character: a textured 3D bust with a rigged mouth (open, wide, narrow, pucker, upper lip raised), blinking eyes and a small head sway, driven by the game's own mouth groups so it talks in step with the original pixel-art head.

The pipeline, one character at a time:

| # | Stage | Tool | Output | Cost |
|---|---|---|---|---|
| 1 | HD portrait from the game's frame 0 | Tripo image-to-image (or the 2D studio) | `img2img/<CODE>-tripo.png` | 5 credits |
| 2 | 3D bust from the portrait | Tripo image-to-model | `<name>/model.glb` | 60 credits |
| 3 | Face landmarks, scale, FLAME fit | `view_tripo.py`, `prescale.py`, `prep_tripo.py`, `fit.py` | `flame-fit/<dir>/fit.pt` | ~1 min on the GPU |
| 4 | Mouth rig | `build.py` | `<name>/<name>-flame.glb` (~140 MB) | ~2 min on the GPU |
| 5 | Talking page | `make_page.py` + a character config | `index-<name>.html` | — |

1 Tripo credit = $0.01. A new character costs about 65 credits.

## 2. Stage 1: the HD portrait

Tripo's **image-to-image** endpoint repaints the pixel art at 2048 px while keeping its composition. It beat our SDXL studio pass at staying faithful (no invented epaulettes or eye colours) and needs no tuning.

- Input: the game's `f00` head frame enlarged 6× with nearest-neighbour (`heads/<CODE>-f00.png`).
- Script: `tripo/tripo_img.py IMAGE OUT.png "PROMPT" [--model seedream_v5] [--size 2048x2048]` (`POST /v3/generation/image-to-image`, default model `seedream_v5`).
- Prompt: "Turn this 1992 pixel-art game portrait into a high-detail … HD portrait. Keep exactly the same framing, pose, head angle, gaze, background and colours." + the character's description from `recipes/heads.json` + the style:
  - **Photographic:** "photorealistic … film still from David Lynch's Dune (1984), dim amber light". Used for the current Leto and Jessica heads.
  - **Painted:** "semi-realistic digital painting with visible soft brushwork …, slightly stylized forms like 1980s science-fiction book cover illustration and the original game art … not a photograph, and not a cartoon". Sits between the game art and the photo.
- Keep the size's aspect to the frame's (Jessica's 173×150 frame → `2208x1920`).

Alternatively, stage 2 can start from an approved 2D studio portrait (`versions/head__<CODE>__f00/…png`), as the first Leto and Jessica heads did.

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
4. **Split, not cut.** Every triangle the lip line crosses is split exactly on the line; the new points get two copies, one riding each lip, so both lip edges follow a smooth curve.
5. **Smooth motion.** Each lip's motion is averaged over that lip only (never across the slit); past the corners the two lips blend back into one surface (2 mm inside to 6 mm past each corner), so the corners taper closed. Away from the mouth the motion is smoothed too, which removes ragged bands along the jaw.
6. **Inner lips.** A two-row strip curls from each lip edge into the mouth so open lips look rounded. The upper strip tucks up steeply (`UPPER_STRIP`, default 0.6× back, 2.2× curl) so it never hangs in front of the teeth.
7. **The inside.** A mouth bag (a dark cup), a tongue, and upper and lower rows of teeth on a dental arch; the lower parts ride the jaw. Side teeth darken towards the back.
8. **Shading.** The lip edges and the skin next to them darken as the mouth opens (a colour morph on `open`), so a closed mouth has no smudge.
9. **Eyeballs (optional).** With `DIR/eyes.json` the painted eyes are replaced by real eyeballs that the page can turn: see [§5a](#5a-eyeballs-and-gaze).
10. **Output.** A GLB with sparse morph targets on the Tripo mesh and separate meshes for the bag, tongue and teeth, each with an `open` target.

## 5a. Eyeballs and gaze

Tripo paints the eyes onto the face's surface: there is no eyeball to turn, and sliding the texture to move the iris distorts it. `build.py` therefore cuts a socket and puts a real eyeball in each eye.

**1. Measure each eye opening** on a close front render (`view_tripo.py MODEL-n.glb OUT.png front y0 y1 z0 z1 900`, about 3 cm tall around the eye) and write `DIR/eyes.json`, one entry per eye. `l` is the eye at positive z, on the **viewer's left** (the character's right); `r` is on the viewer's right (the character's left). Say which one you mean as seen by the viewer, to avoid moving the wrong eye.

| Key | Meaning |
|---|---|
| `inner`, `outer` | the two corners `[z, y]`, **where the white of the eye ends**, not on the lashes |
| `mid`, `top`, `bottom` | the middle's z, the upper lid's y and the lower lid's y there |
| `radius_abs` | the eyeball's radius in model units: give **both eyes the same value** (Leto and Jessica photo: 0.0255) |
| `iris`, `pupil` | their angular radius on the ball (radians); pick `iris` = asin(iris radius / `radius_abs`) so both irises match |
| `iris_rgb`, `sclera_rgb` | colours (otherwise sampled from the painted eye; a hand-set value is safer: Leto `[78, 88, 96]`, Jessica `[96, 62, 34]`, sclera `[138, 126, 110]`) |
| `behind` | how far behind the lid surface the ball sits (m): negative brings it forward, against the lids (Jessica −0.002 and −0.0035) |
| `shift_z` | moves the ball (and where its pupil shows) along the face, for fine placement; leave unset when the corners are right |
| `margin`, `margin_low` | how far outside the lid lines painted-eye colour is also cut (default 2.5 mm; `margin_low` below the lower lid, 0 where lashes there are mistaken for eye) |

Measure each corner where the **white ends**: lashes, the dark crease at the inner corner and the shadow at the outer one all look like part of the eye at a glance. Jessica's inner corner first sat 1 cm into the crease: the ball centred in that opening sat off towards the nose, and no nudge fixed it until the corner was re-measured. An outer corner past the white shows a sliver of eyeball beyond the lid.

The early lesson: corners measured out on the lashes made one opening 70 % too wide, so the eyeballs came out different sizes and every later nudge just compensated. Measure carefully once, use one radius for both eyes.

**2. The build**, per eye:
- the lids are two parabolas through the corners and the middle points;
- every surface point inside the opening, plus painted-eye colour just outside it (not skin-coloured), is removed, and any small flake around the eye left unattached to the face goes too (no floating lash or lid pieces);
- a sphere of `radius_abs` is centred on the opening's middle, just behind the lid surface, with a painted iris texture (pupil, radial streaks, darker limbal ring, toned-down white);
- the build prints a `DIAG` line per eye: the lid surface, the upper and lower rim depth, the ball's front and how far it stands out past each rim, the opening's width and height, and the ball's position against the opening's middle. Compare the two eyes there: the widths should be close, and the ball should stand out by about the same average amount (Jessica: 5.2 / 5.7 and 3.4 / 7.4, a heavier upper lid on one side); match them with `behind`;
- the GLB gets a node `eyes` (the bust's transform) with `eye_l` / `eye_r` children; each carries `extras.fwd` (the ball's rest axis) and `extras.open` (the opening's middle).

**3. Gaze on the page** (`index-flame.html`, every frame):
- *neutral*: the direction that puts the pupil in the middle of the opening seen from the page's default camera;
- *look at the viewer*: by default (`window.lookAtCamera = true`) each eye turns from neutral towards the camera, clamped to a human range of about 25° sideways and 12° up or down (`window.EYE_LIMIT = [0.44, 0.21]` radians; the busts are turned ~22° from the viewer, and a tighter limit left the eyes short of the viewer, focused somewhere behind them: the blank stare), so orbiting round the bust the eyes follow and then stop instead of rolling into the corners;
- *focus*: the eyes aim at a point on the line from between them to the camera, `window.EYE_FOCUS` (default 4) times farther than the camera. At the camera's true distance (about 1 m at the head's scale) they converge ~3.8° and look cross-eyed; at 4× the lines of sight converge under 1°, like someone looking at you from across a table;
- *saccades*: small quick glances off the viewer and back (on their own clock, so they continue when talking is paused);
- the eyes follow the head sway and tuck back behind the closing lids; with `lookAtCamera = false`, `window.gaze = [yaw, pitch]` sets them directly.

**Expression around the eyes.** `build.py` also adds three morph targets solved in FLAME's expression space with the mouth, nose and scalp held still: `brow_down` (brows lowered, the inner ends more and drawn together), `brow_up` and `squint` (lower lids raised). The page keeps a resting level of lowered brows and squint, presses both harder on stressed (wide-open) syllables and lifts the brows briefly at some phrase starts. Strengths per character in `CHAR.json` `expr` (`brow`, `squint`, `emph`, `lift`; Leto 0.4 / 0.35 / 0.4 / 0.35, Jessica 0.25 / 0.25 / 0.3 / 0.45). The pupils are 40 % of the iris width: smaller pupils read as a blank stare.

**4. Check** with the mouth closed and `window.force = {open: 0, blink: 0}`: a close view of each eye (white sliver past a corner = corner too far out; gap round the ball = `behind` too far back; missing lid piece = lower lid line too low or `margin_low`), a full blink (`blink: 1`), and an orbit.

## 6. Stage 5: the talking page

`flame-fit/make_page.py CHAR.json OUT.html` makes the page from the Leto template (`index-flame.html`). The config (`tripo/leto-photo.json`, `jess-photo.json`, …):

| Key | Meaning |
|---|---|
| `code`, `name`, `size` | the head's code, its display name, the frame's size in pixels |
| `glb`, `landmarks` | the rigged GLB and the landmarks file (places the eyelids and the camera) |
| `shapes` | per mouth group: `[open, wide, round, upper]`, read by eye from the pixel art |
| `blink`, `half_blink`, `eye_box` | the game's eye groups and where they draw |
| `blink_w`, `blink_h` | the blink area's width and height × the default (Jessica photo: 0.85, 1.0) |
| `eyes_gaze` | the eyeballs' `[yaw, pitch]` when `lookAtCamera` is off |
| `lids` | per eye three `[z, y]` points on the lower lid (inner, middle, outer): the blink shuts on that curve (Jessica photo) |
| `yaw` | the whole bust turned (radians), no deformation; unset keeps the portrait's own turn, which matches the sprite |
| `teeth` | `[r, g, b, env]` tint for the teeth, for a darker portrait (Leto photo: `[0.36, 0.3, 0.24, 0.12]`) |
| `camera_distance`, `target_dy` | framing |
| `intro` | the paragraph under the title |

The page plays a 60-second syllable timeline built from the character's mouth groups (consonants = nearly closed shapes, vowels = open ones), drives the five morph targets and a lip seal that fades as the lips part, blinks with a shader on the eyelids, and sways the head. A button records 8 s as WebM. `window.force = {open, wide, round, upper, blink}` holds a shape for checking.

To add a character to the preview page, add a line to `PAGES` in `tripo/preview.html`.

## 7. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| A pinch or peak on a lip when it opens | the crease search went into the other lip | lower `CREASE_MM` (Leto photo: 2) |
| A ragged edge on the upper lip | `CREASE_MM` too low: the cut sits in the lower lip | raise it again |
| A flap hanging in front of the upper teeth | the upper inner-lip strip | `UPPER_STRIP` (curl it up more) |
| Spikes or holes in the lips when open | neighbouring vertices bound to different FLAME parts | already handled by the per-lip smoothing; check the landmarks |
| White specks at the corners | teeth seen through holes in the lip | already handled by the closed corners; check the mouth corner picks |
| Teeth too bright | the portrait's light is dimmer than the template's | `teeth` in the config |
| Eyes of different sizes, or the pupils off-centre | eye corners measured on the lashes | re-measure the openings, one `radius_abs` for both (§5a) |
| Floating pieces round an eye | painted lash or lid left in front of the ball | handled by the socket clean-up; widen `margin` |
| A notch in the lower lid | lower lid line too low, or lashes cut as eye | raise `bottom`; `margin_low` 0 |
| The blink closes only part of the eye | eye corners picked too close together | re-pick on a close render; then `blink_w` |
| The blink drags a wide band of skin | blink area too wide | `blink_w` below 1 |
| A ragged band along the jaw | uneven binding under the chin | handled by the far-field smoothing |

## 8. Files, environment and rules

- **Where things are.** Everything lives **outside the repo**, under `Cryogenic-Web/portrait-proto/studio/tripo/`: the scripts (`tripo.py`, `tripo_img.py`, `tripo_rig.py`, `flame-fit/*.py`), the per-character work (`flame-fit/<dir>/`), the models (`leto-hd/`, `jess-hd/`, `leto-photo/`, `jess-photo/`, …) and the pages. The FLAME files are in `Cryogenic-Web/portrait-proto/flame/model/` (`flame2023_Open.pkl`, `FLAME_masks.pkl`), downloaded by the user from the FLAME site.
- **Environment.** The Windows NVIDIA machine; Python `%LOCALAPPDATA%\DuneHD\env\Scripts\python.exe` (torch with CUDA). The FLAME loader in `flame.py` needs neither chumpy nor scipy. Tripo needs `TRIPO_API_KEY`.
- **Serving the pages.** From `portrait-proto/studio`: `python -m http.server 8790 --bind 127.0.0.1`, then open the [preview page](http://127.0.0.1:8790/tripo/preview.html).
- **Never commit or share game-derived files:** the portraits, busts, GLBs, renders and pages all come from the game's art. They stay on the user's machine. Only this description belongs in the repo.
- **FLAME licence.** FLAME 2023 Open is CC-BY 4.0: anything shared that uses it must credit FLAME (Li et al., "Learning a model of facial shape and expression from 4D scans", SIGGRAPH Asia 2017). The FLAME files themselves are never committed.
- **Credits.** Ask before spending Tripo credits on new characters.

## 9. Related experiments

The same Tripo account built rigged full-body figures placed on the HD rooms: an Atreides guard and rifle (`guard-pose.html`, rigged with Tripo's auto-rig, 25 credits; posed in code with arm IK and hand contacts), the throne room with the guards and Leto (`leto-throne.html`), and the dining hall with depth-based occlusion from Depth Anything V2 (`dining.html`).
