# The globe and the map in high definition

**State (2026-11-26): a working prototype outside the engine.** One HD picture of the planet's whole surface, and a page that draws the globe and the flat map from it live, with the game's place, size, movements and click behaviour. Nothing is drawn by the Rust engine yet.

- **Prototype:** the globe at [http://127.0.0.1:8790/tripo/globe/globe.html](http://127.0.0.1:8790/tripo/globe/globe.html), the flat map at [globe.html#map](http://127.0.0.1:8790/tripo/globe/globe.html#map) (serve `portrait-proto/studio` on port 8790, as for the preview page). The preview page's HUD tab has a button for each.
- **Files:** `Cryogenic-Web/portrait-proto/studio/tripo/globe/` (outside this repo). The pictures are game-derived and are in no repo; the scripts and the recipe can go to the private repo like the rooms' and the HUD's.
- **Related:** the rooms and the HUD are in [talking-heads-3d.md](talking-heads-3d.md), sections 10 and 11.

## 1. How the game draws its planet

Read from the decompile worktree (`crates/dune-rec/src/hand/globe.rs`, `map.rs`, `files.rs`, the lifted driver in `generated_vga.rs`), the listing's data tables, the game data and a draw trace.

- **One surface for every view.** `MAP.HSQ` is 50,681 cells, one byte each. The globe, the main map and the small maps all read it. A cell's low nibble is a shade 0 to 15 (the relief you see); bits 4 and 5 are a state the game changes (vegetation, water); bit 6 marks a location's cell.
- **Stored ring by ring.** `TABLAT.BIN` gives, for each of 99 rows from the equator to a pole, where the ring starts and its half width: 199 cells at the equator down to 3 (a sinusoidal map, 197 rows from -98 at the top to +98). The equator's ring starts at byte 62FCh; a negative row's ring starts that much before it.
- **The globe is drawn by the graphics driver from tables**, with no sprite and no trigonometry at run time (slots 23 and 24; `GLOBDATA.HSQ` holds the per-pixel tables). The disc is 128 x 110 pixels centred at (160, 80): a circle on the game's 4:3 screen, whose pixels are a fifth taller than wide.
- **Rotation and tilt.** Rotation is a number of cells, 0 to 397, shifting every ring in proportion to its width; the buttons move it by 32. Tilt moves by 8 rows. The driver draws one screen row a tick and adds one cell of rotation for each globe completed.
- **Colour** is the shade plus 16 (palette 10h to 1Fh); the results view tints cells by their state. No lighting.
- **On the globe only the blue arrow** at the player's place (`ICONES` 54) is drawn. Locations, troops and spice are on the flat maps.
- **Clicks go back through the same tables** to a map position, and the flat map opens there.
- **The main map** shows each cell as 4 x 4 pixels, smoothed, each row as wide as its ring about the mark's meridian (slot 29), in the view (4,4)-(316,148); then icons from `ONMAP`, markers and troops. The place view colours regions from a second file of the same layout, `MAP2.HSQ`, which holds region numbers.
- **Around the globe:** two side panels and a ring from `FRESK`, on a field of colour F0h.
- **In HD today** all these driver draws come out low-resolution: the HD layer does not model slots 22, 23, 24, 29 and 42.

## 2. The approach

1. **The game's cells stay the data.** Positions, clicks and the game's rules keep using them.
2. **One HD picture of the whole surface** is their skin: a flat map, longitude across and latitude down, that any view can sample.
3. **The engine draws the views**, as the driver does, at any resolution: for each pixel, the latitude and longitude under it, then a lookup in the HD picture. Globe and maps always agree because they read one picture.
4. **What the game changes** (vegetation, water, the results' tints, region colours) is drawn over it from the low-resolution state, in the same coordinates.

## 3. What was made

| Step | Script | Result |
|---|---|---|
| The surface out of the game data, unwrapped | `globe_data.py` | `surface-shade.png`, `surface-state.png`, `surface-game.png` (398 x 197), `surface.json` (rings, the game's 16 colours from a trace) |
| A generated picture of it | `gemini_img.py`, recipe `globe-recipes.json` | `surface-hd.png`, 5820 x 2880 (third take) |
| The HD skin | `globe_surface.py` | `surface-final.png`, 4096 x 2048, about 10 pixels a cell |
| The projection, checked against the game | `globe_render.py fit` | `fit.json`, `fit.png` |
| The globe screen without its globe | recipe `plate` | `plate-hd.png`: the Harkonnen and Atreides panels as marble bas-reliefs on the blue field |
| The panels with our own characters | recipe `plate_people`, `globe_plate.py` | `plate-people.png`: Leto, Jessica and our hawk on the Atreides panel, the Emperor and the floating Baron on the Harkonnen one, in the same marble |
| The map's icons and the globe's arrow | recipe `icons`, `globe_icons.py` | `icons/ONMAP-NNN.png`, `hud/pieces/ICONES-054.png` |
| Every place of the game | read out of `DNCDPRG.EXE` | `locations.json`: 70 records |
| The HUD panel in marble | recipe `panel_marble`, `globe_assets.py` | `panel-marble-frame.png`, `web/panel-marble.png` |
| The page's pictures | `globe_assets.py` | `web/` (surface, plate, the HUD panel in its globe state with the head) |
| The prototype | `globe.html` | the globe and the map, live |

**The surface took three takes, and the generated picture is not used as it is.**
- From the pixel art enlarged as it is, the model kept the pixel steps as blocky edges.
- From a smoothed copy, it drew contour lines along the steps of the 16 shades.
- From a strongly blurred "colour guide" with a prompt asking for a photographic mosaic, the geography held and the rock regions are photographic, but of a dozen craters two came back, the regions' edges have halos and the colours changed.
- So `globe_surface.py` builds the skin from both: **colour** is the game's own shade, interpolated between cells, through the game's 16 colours (every crater and swirl is there, in place, in its colour); **detail** is the generated picture's fine texture, multiplied in, faded out at the left and right edges where the map joins itself. Its settings are in the recipe (`final`).

**The projection matches the game.** An orthographic sphere by formula, fitted to a captured frame (rotation 63 cells, tilt -31 rows): the same features in the same places, 69% of the disc's pixels alike, the rest off by a pixel or two. The player's arrow lands within about a pixel of the game's. The formula stands in for the driver's tables; an exact match needs those tables (`GLOBDATA.HSQ` and the tilt table), which is a known remaining difference.

**The prototype** (checked in Chrome by reading its state and saving its renders):
- The disc at the game's place and size, drawn at 8 times the game's resolution, on the HD plate, over the HUD panel in its globe state.
- Drag to turn and tilt; four buttons move by the game's steps (32 cells, 8 rows); it turns slowly by itself. On the map, dragging or the buttons scroll.
- The blue arrow at the palace, hidden when it is behind the planet. On the map: every place of the game's table with its icon, and the hawk pointer. The map shows about 78 cells by 36, so they are in view only near the player's place: **Map at my position** (or `#map`) opens it there, as the game's SEE DUNE MAP does.
- Over the disc the command list becomes SEE MAP OF THIS AREA; a click opens the flat map at that point, drawn from the same picture at 4 pixels a cell; EXIT MAPS comes back.
- A readout of the map position under the pointer, in the game's units.
- Switches: the game's own cells in place of the HD surface; the game's resolution; shading towards the rim with a thin atmosphere (not in the game).
- With the game's cells at the game's resolution it reproduces the captured frame's disc.

Saved renders: `globe/shots/compare.jpg` (the game's frame; HD; the game's cells in the prototype; another view with rim shading), `compare-map.jpg` (the game's map beside the prototype's, with the icons).

**The side panels with our characters (user).** The first plate's faces and emblems were the model's own. Editing the whole wide picture with five reference pictures failed twice (without an aspect ratio it came back square; at 16:9 it moved both panels to the middle). What works: each panel cut out alone as a 2:3 picture that it fills, edited with its own references (our HD portraits, the hawk's vector master rendered), and pasted back. The Baron came out in his suspensor harness, without his robe; the small emblem low on the Atreides panel stayed a V sign.

**The icons.**
- **On the globe the game draws only the blue arrow** (`ICONES` 54, 10 x 16, its lower left corner on the place). It is redrawn from the sprite's outline as a polygon in its two blues, not generated.
- **On the flat map** the game draws each place of its location table with an icon chosen by its kind (`location_icon`: sprite 7Ah on this map, plus one for each of 20h, 21h, 28h, 30h that the kind reaches), centred on the place, then markers and troops. The pointer on the player's place (sprite 4Ch, 13 pixels above it) is our hawk.

  | Kind (byte 8) | Place | `ONMAP` sprite | Size | Our icon |
  |---|---|---|---|---|
  | below 20h | sietch (42) | 122 | 27 x 14 | the entrance of scene DS 0 |
  | 20h | the palace of the Atreides | 123 | 36 x 30 | the palace of scene DP 2, tinted red |
  | 21h to 27h | village (5) | 124 | 17 x 17 | the houses of scene VIL 3 |
  | 28h to 2Fh | fortress (21) | 125 | 17 x 15 | the fort of scene DF 3, tinted blue |
  | 30h and up | the Harkonnen palace | 126 | 36 x 30 | the citadel of scene DH 0, tinted blue |

  Sprite 127 is 122's picture in another palette (the trace names it 127; the inventory's 122 is the sietch in the wrong colours).
- **The location table** is in the executable's data, 70 records of 1Ch bytes from DS:0100h (file offset F7B0h in `DNCDPRG.EXE`): word 2 the x (a fraction of the turn), word 4 the y (a row), byte 8 the kind, byte 0Ah the flags (80h: not yet found, 65 of the 70 at the start). The prototype places all of them, with a switch for those known at the start only. Checked against a draw trace of the map at the start of the game: the palace at (142,61) and three sietches at (223,69), (228,125), (194,9) come out within a pixel.
- **How they are made:** sprites enlarged on a flat key colour in one sheet (green; magenta for what is green itself), one Banana Pro call a sheet, each cut out by the key's hue and fitted in a box of its sprite's proportions. They came out as painted miniatures.
- **The five places were each redone alone, with one of our own scenes as the style** (user): one call each at 2K, the sprite enlarged on green as the first picture and the scene as the second, asked for the scene's architecture as a compact map marker. The Harkonnen ones are tinted blue afterwards, keeping their light and dark (`tint` in the recipe), as the game's own icons are blue; the palace's red was asked of the model.
- **Made:** the five places, the harvester, two ornithopters, three plants. **Not made:** the troop figures (rigged 3D figures, one per tint: their HD references are below). The small maps' place icons (`ICONES` 58 to 63) are not made either.

**The troop figures (2026-11-26, HD references only, no 3D yet).** The map's troop markers are `ONMAP` 0 to 96, one figure per tint (user):

| Tint | Who | Sprites | Postures |
|---|---|---|---|
| yellow | spice collectors | 0, 3-26 | walking (3-18), shovelling (19-22), poking the ground with a spike (23-26) |
| red | Fremen troops | 1, 27-54 | walking (27-42), training with a knife (43-53), prisoner (54) |
| green | planters | 2, 55-78 | walking (55-70), at a plant (71-74), with a tool (75-78) |
| blue | Harkonnen troops | 79-95 | walking and standing |
| outline | unknown | 96 | |

- **The plan:** one rigged 3D figure per tint (Tripo image-to-model and auto-rig), the postures from Mixamo clips, the tools (shovel, spike, knife, planting tool) as separate props.
- **The references** are in `tripo/troops/`: `troops-recipes.json` (the texts), `troops_gen.py` (the kind's sprites enlarged as a colour guide, plus style pictures: Chani's and Stilgar's full-body images for the stillsuits, the Harkonnen soldier's for his motifs), `KIND.png`. Each is an open A-pose with empty hands on flat grey, as for the guard. The collector and the planter wear plain stillsuits, the Fremen troops an armoured one, the Harkonnens blue steel armour. The unknown figure is not generated: the collector's picture as a dark silhouette with a rust rim.
- **Review page:** [troops/troops.html](http://127.0.0.1:8790/tripo/troops/troops.html) (also the preview page's HUD tab, **Map troops**): each figure beside its sprites by posture, with a notes box saved to `tripo/out/troops-notes.json`. A try beside the keeper: `troops_gen.py KIND --take v1` (the page shows `KIND-v1.png` to `-v9.png` as buttons).
- **Later takes (user):** the collector in glossy moulded leather, not cloth (it came back on black, at the full 2304 x 3072); the planter with a green headband and a cloth hanging from it down the back. First takes kept as `KIND-v1.png`. The page adds a time stamp to its pictures' addresses: a regenerated picture keeps its name and the browser showed its cached copy.
- **3D models and rigs (2026-11-26):** `tripo.py troops/KIND.png troops/KIND-3d`, then `tripo_rig.py TASK_ID troops/KIND-3d` (the task id from `task.json`): `model.glb` and `rigged.glb` for the four figures, each a biped with the 65-bone Mixamo skeleton, 85 credits a figure. The page shows each `rigged.glb` live under its picture (`figure-view.html`). Built into the meshes: the red figure's scabbard at the hip, the planter's back cloth and scarf. Not done: the clips for the postures, the props, the unknown figure's flat material, the renders for the map.
- **The walk (2026-11-26):** `troops/troops_walk.py KIND` retargets Tripo's preset walk onto the rig (`POST /animations/retarget`, `animation` `preset:walk`, 10 credits) and saves `KIND-3d/walk.glb`: one clip of 2.4 s on the Mixamo bones. No Mixamo download was needed for it.
- **The markers' view.** In all four directions the game draws the figure from above with the head at the upper left and the feet at the lower right; two directions show the face and two the back, so the four are read as diagonal headings. [troops/walk.html](http://127.0.0.1:8790/tripo/troops/walk.html) (HUD tab, **Troops walking**) shows, per kind and direction, the game's four frames beside the 3D figure in the same square of 20 game pixels, walking on the spot (the hips held over their starting place). The 3D walk plays smoothly at the clip's own pace (`speed`, 1 by default) and is not matched to the sprites' four frames, only to their view and direction (user); the sprite loops its own frames beside it. With an overlay of the sprite, the render at the game's size and at 4 to a game pixel. Its view is an orthographic camera in front of the figure, raised by `pitch`, the picture rolled by `roll`, the figure `size` game pixels tall and turned by `yaw` per direction: first values in `troops/walk-settings.json` (pitch 40, roll 45, size 24, yaw 135, 45, -45, -135, set by eye on one frame); the page's sliders save to `out/troops-walk.json`, which it reads first. Which heading is left and which right within each pair is a guess. `window.shot(name, frame)` saves the eight panels through the save server (a hidden tab gets no animation frames and cannot be screenshotted).
- **Shovelling (2026-11-26, sprites 19-22):** the shovel is `troops/props/shovel/model.glb` (image from Tripo's generator, 5 credits, then image-to-model, 60), standing upright with its D-grip at the top. The page's **Shovelling** action (`walk.html#collector/shovel`, one panel: the sprites are one view, four frames; with Mixamo's Digging a turn of 45 matches them, the shovel across to the right at the bottom of the swing) holds it in both hands every frame: the grip in the top hand's palm, the shaft through the other palm, the fingers closed round it (each finger bone turned about the shaft until its end is at `wrap` from it). Settings in `act.shovel`. The clip the user chose is Mixamo's **Digging**: downloaded by the user into `troops/anim/Digging.fbx` (FBX, without skin), the page retargets it onto Tripo's rig (same bone names; our A-pose first aimed into the clip's rest pose bone by bone, then each bone takes the source bone's turn from rest in world space; the hips' movement scaled). Downloaded 2026-11-26 through the browser extension on the user's Mixamo account (the character then selected there, FBX Binary, without skin, 30 fps, 146 frames); it plays correctly, the shovel in the hands, blade down. Without that file Tripo's `preset:dig` (`KIND-3d/dig.glb`, 10 credits: a kneeling dig with open hands) stands in.
- **Poking the ground (2026-11-26, sprites 23-26):** Mixamo has no such clip (searched: stab, poke, plunge, spear, staff, stick, pole, pound, the farming pack; the nearest, Bayonet Stab, thrusts forward), so it is made on the page (`walk.html#collector/poke`, **Poking**): the body holds one frame of Digging (`still` 0.4 s, upright, the tool held upright in both hands), the stick strokes along its own line (lifted, driven into the sand, held, pulled out: `strokeAt`), the back bends with the downstroke, and both arms follow the stick by two-bone IK (`ik`), the fingers closed round it. The probe: a picture from Tripo's generator (`troops/props/spike.png`, a steel rod with a conical point, two leather grips and a knob, 5 credits); its Tripo model (`props/spike/model.glb`, 60 credits) came out a flat slab 14 % of its length wide and is not used: the page builds the probe from the picture's parts (`buildSpike`), its length set so that the point is at the ground at rest. Settings in `act.poke`; a turn of 0 is a first guess against the sprites.
- **Generator:** Google's prepaid credits were used up (HTTP 402), so these went through Tripo's image generator (`provider` in the recipe; `seedream_v5`, 5 credits each, asked at 2304 x 3072 and returned at 768 x 1344, one request at a time: four at once got HTTP 429). Prompts are kept under Tripo's 1,800 characters.

**The HUD panel in marble (user).** The game turns its panel blue-grey on the globe and map screens. Ours is an edit of the whole frame (the HUD base with the globe block and the head up, `hud/globe-frame.png`): only the material changes, to pale veined blue-grey marble with steel fittings and a dark blue slate; the geometry is kept, so the panel is cut as before and takes the sandstone panel's alpha. The frame of the command panel and the arrow cross stayed copper. Given the side panels' plate as a second picture, the model drew those panels into the frame and shrank the console; the marble is described in words instead. The prototype shows the marble panel, with a switch back to the sandstone one. The panel's overlays (lit arrows, button, book) exist in sandstone only.

**Cost:** seventeen calls to Google, about $3.60 (three surface takes, the plate, four for the panels of which two were unusable, two icon sheets, five place icons at 2K, two for the marble panel of which one was unusable). All of it through Google directly (`gemini_img.py`); nothing here uses Tripo.

## 4. Known limits

- **Sharpness.** 10 pixels a cell is plenty for the globe (a cell is about 2.6 screen pixels at 8 times), but the main map shows a cell 32 pixels wide at that size, so it is soft. A sharper map needs a larger skin, generated in tiles.
- **The sand reads soft and a little glowing**, since most of its detail is the smoothed game shade. The dark rock is better.
- **The poles** are stretched in a flat map and pinch on the globe. They want two small round pictures of their own.
- **The screen's shape.** The page shows 320 x 200 on a 4:3 screen, as the game was seen, so the disc is round. The HUD mock-ups were made with square pixels; the panel is simply stretched here.
- **The panel's left block** shows the globe without the four small arrows the game draws there, and the marble panel has no overlays of its own yet.
- **Speed of the turning** is an estimate (the driver's tick was not measured).

## 5. Next

1. **The rest of the map's icons**: the troops (probably rigged 3D figures) and the small maps' place icons. The places' state in a running game (found or not, taken by the Harkonnens) comes from the same records in a save.
2. **The game's changes over the skin**: the state bits as soft masks (their meaning in game terms is not named in the code; a late save will show them), the results view's tints, the place view's region colours from `MAP2.HSQ`.
3. **A larger skin in tiles**, and the two polar pictures.
4. **In the engine:** the driver's globe and map slots replaced by this renderer in the HD layer, using the driver's own tables for an exact match; the side panels' sliding and the results bars.
5. **The small maps** (ornithopter, 1 pixel a cell) from the same skin, with their polar inset.
