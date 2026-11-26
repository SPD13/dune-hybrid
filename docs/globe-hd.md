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
- The blue arrow at the palace, hidden when it is behind the planet. On the map: the palace, the three sietches and the hawk pointer. The map shows about 78 cells by 36, so they are in view only near the player's place: **Map at my position** (or `#map`) opens it there, as the game's SEE DUNE MAP does.
- Over the disc the command list becomes SEE MAP OF THIS AREA; a click opens the flat map at that point, drawn from the same picture at 4 pixels a cell; EXIT MAPS comes back.
- A readout of the map position under the pointer, in the game's units.
- Switches: the game's own cells in place of the HD surface; the game's resolution; shading towards the rim with a thin atmosphere (not in the game).
- With the game's cells at the game's resolution it reproduces the captured frame's disc.

Saved renders: `globe/shots/compare.jpg` (the game's frame; HD; the game's cells in the prototype; another view with rim shading), `compare-map.jpg` (the game's map beside the prototype's, with the icons).

**The side panels with our characters (user).** The first plate's faces and emblems were the model's own. Editing the whole wide picture with five reference pictures failed twice (without an aspect ratio it came back square; at 16:9 it moved both panels to the middle). What works: each panel cut out alone as a 2:3 picture that it fills, edited with its own references (our HD portraits, the hawk's vector master rendered), and pasted back. The Baron came out in his suspensor harness, without his robe; the small emblem low on the Atreides panel stayed a V sign.

**The icons.**
- **On the globe the game draws only the blue arrow** (`ICONES` 54, 10 x 16, its lower left corner on the place). It is redrawn from the sprite's outline as a polygon in its two blues, not generated.
- **On the flat map** the game draws the places from `ONMAP` (122 + kind when zoomed: 122 dome, 123 palace, 124 hut, 125 bunker, 126 fortress, 127 sietch), then markers and troops. A draw trace of the map at the start of the game gives the palace at (142,61) and three sietches at (223,69), (228,125), (194,9) (top left corners, the mark on the palace). The prototype works their map positions back from those and places the HD icons with the same projection, so they scroll with the map. The pointer on the player's place (sprite 4Ch, 13 pixels above it) is our hawk.
- **How they are made:** sprites enlarged on a flat key colour in one sheet (green; magenta for what is green itself), one Banana Pro call a sheet, each cut out by the key's hue and fitted in a box of its sprite's proportions. They came out as painted miniatures.
- **The palace and the sietch were redone alone, with our own scenes as the style** (user): the palace as in scene DP 2 (the cluster of rounded towers on its stepped base), its stone tinted red for the Atreides; the sietch with the entrance of scene DS 0 (the layered golden rock with its low arched mouth).
- **Made:** the six places, the harvester, two ornithopters, three plants. **Not made:** the troop figures (user: not for now; they walk and work in many frames, so a rigged 3D figure per kind, as for the palace guard, is the likely way). The small maps' place icons (`ICONES` 58 to 63) are not made either.

**Cost:** twelve calls to Google, about $2.70 (three surface takes, the plate, four for the panels of which two were unusable, two icon sheets, and the palace and the sietch at 2K).

## 4. Known limits

- **Sharpness.** 10 pixels a cell is plenty for the globe (a cell is about 2.6 screen pixels at 8 times), but the main map shows a cell 32 pixels wide at that size, so it is soft. A sharper map needs a larger skin, generated in tiles.
- **The sand reads soft and a little glowing**, since most of its detail is the smoothed game shade. The dark rock is better.
- **The poles** are stretched in a flat map and pinch on the globe. They want two small round pictures of their own.
- **The screen's shape.** The page shows 320 x 200 on a 4:3 screen, as the game was seen, so the disc is round. The HUD mock-ups were made with square pixels; the panel is simply stretched here.
- **The panel is our sandstone one**; the game recolours it blue on this screen. Its left block shows the globe without the four small arrows the game draws there.
- **Speed of the turning** is an estimate (the driver's tick was not measured).

## 5. Next

1. **The rest of the map's icons**: the troops (probably rigged 3D figures), the small maps' place icons, and the places of a later game (the location table at DS:0100h, 1Ch bytes a record, gives every place's position and kind; the prototype has only the four of the start).
2. **The game's changes over the skin**: the state bits as soft masks (their meaning in game terms is not named in the code; a late save will show them), the results view's tints, the place view's region colours from `MAP2.HSQ`.
3. **A larger skin in tiles**, and the two polar pictures.
4. **In the engine:** the driver's globe and map slots replaced by this renderer in the HD layer, using the driver's own tables for an exact match; the side panels' sliding and the results bars.
5. **The small maps** (ornithopter, 1 pixel a cell) from the same skin, with their polar inset.
