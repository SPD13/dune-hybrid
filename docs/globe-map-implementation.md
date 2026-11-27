# The globe and the flat map: how to implement them in the game

This is the specification for drawing the globe screen and the flat map screen in HD in the game, written from the prototype that does it today (`Cryogenic-Web/portrait-proto/studio/tripo/globe/globe.html`, in the private repo `SPD13/dune-talking-heads`). The prototype was matched against the game's own code (the decompile worktree's `hand/globe.rs`, `hand/map.rs`, the lifted DNVGA driver in `generated_vga.rs`) and checked against captured frames. The approach, the assets and how they were made are in [globe-hd.md](globe-hd.md); this document is what an implementation needs.

**Principle.** The game's data and logic stay as they are: the map cells, the positions of places and troops, the mark, the clicks. Only the drawing changes. Every coordinate below is in the game's own units, so the HD drawing can sit on top of the original screen state and stay in step with it.

## 1. Units and data

| Thing | Unit | Where it lives |
|---|---|---|
| Screen | 320 x 200 game pixels; the picture above the panel is 320 x 152 | |
| A map position's x | a 16-bit fraction of a turn, `u = x / 65536` | location records word 2; the mark `[197Ch]` |
| A map position's y | a row, -98 (top) .. +98 (bottom), 0 the equator | location records word 4; the mark `[197Eh]` |
| A row's ring | `L(row)` cells round the planet: `2 * half_width` of TABLAT row `abs(row)`; 398 at the equator, 6 at row 98 | `TABLAT.BIN` (99 rows of 8 bytes, words big-endian; word 0 the ring's start, word 1 the half width) |
| A cell | one byte of `MAP.HSQ`; low nibble the shade 0..15, bits 4-5 a state, bit 6 a place's mark | the equator's ring starts at byte 62FCh; a negative row's ring at `62FCh - offset` |
| Globe rotation | cells, 0..397 (`[494Ch]`); as `rot = cells / 398` of a turn | |
| Globe tilt | rows, the row at the centre of the disc (`[2460h]`) | |
| A place | a record of 1Ch bytes from DS:0100h, 70 of them, then FFh: word 2 x, word 4 y, byte 8 the kind, byte 9 the first troop's id, byte 0Ah the flags (80h not found, 40h prospected), byte 10h its region, byte 12h its spice | the data segment; in the executable at file offset F7B0h |
| A troop | a record of 1Bh bytes from DS:08AAh (id 1 first), up to DS:0FBBh: byte 1 the next troop's id at the same place, byte 3 the job (low nibble) and flags (40h moving, 80h not yours), word 4 its place (word 0Ch when byte 3 & 3 = 3) | the data segment |
| A region | one byte a cell of `MAP2.HSQ`, the same ring-by-ring layout as `MAP.HSQ`; 77 regions, 0 none | resource 3Ah |

`L(row)` between two whole rows is interpolated linearly (the driver does the same for the lines between rows). Use the game's table, not a cosine: they differ by a few cells near the poles, and icons would drift off their terrain.

**The HD surface** is one equirectangular picture of the whole planet (`globe/surface-final.png`, 4096 x 2048), longitude across from `u = 0` at the left, the game's rows down from -98 at the top: the picture's height `v = (row + 98.5) / 197`. It repeats left to right. It is the game's own shades, enlarged and in the game's colours, carrying generated detail, so every feature is where the cells put it.

## 2. The globe screen

**Layout.** The picture above the panel is the globe screen's plate (`globe/plate-people.png`: the two marble side panels on the blue field). The disc is drawn over it, then the panel (`web/panel-marble.png`, rows 126..200, with alpha where the bronze head rises), then the panel's buttons.

**The disc.** Centre (160, 80), half axes 64 x 55 game pixels: a circle on the game's 4:3 screen, whose pixels are 1.2 times taller than wide. For a point of the disc, with `dx = (x - 160) / 64`, `dy = (y - 80) / 55`, `r2 = dx² + dy²` (outside when `r2 > 1`):

```
z   = sqrt(1 - r2)
t   = tilt * (π/2) / 98                       // the tilt as an angle
yy  = dy * cos t + z * sin t
zz  = -dy * sin t + z * cos t
lat = asin(yy)                                // positive = down, as the game's rows
lon = atan2(dx, zz)
u   = rot + lon / 2π       (repeating)
row = lat * 98 / (π/2)
colour = surface(u, row)
```

This is an orthographic sphere; the game's driver does the same from precomputed tables (`GLOBDATA.HSQ`). Checked against a captured frame (rotation 63 cells, tilt -31): the same features in place, the player's arrow within a pixel. Draw at any resolution; the prototype draws 8 screen pixels to a game pixel, with a soft edge of 1.5% of the radius.

**Map position to the disc** (for the arrow, and anything placed on the globe):

```
lat = row * (π/2) / 98,  lon = (u - rot) * 2π
x0 = cos lat * sin lon,  y0 = sin lat,  z0 = cos lat * cos lon
y  = y0 * cos t - z0 * sin t,   z = y0 * sin t + z0 * cos t
hidden when z <= 0;  else screen (160 + x0 * 64, 80 + y * 55)
```

**What is on the globe.** Only the player's arrow (ICONES 54, 10 x 16), its lower left corner on the player's place, hidden on the far side. The game draws no places, troops or spice on the globe.

**Turning.**
- The game adds one cell of rotation each time it has finished drawing the globe (one screen row a tick). The prototype turns smoothly at 1.6 cells a second; measure the game's tick to match it exactly.
- Dragging turns and tilts it (not in the game).

**The panel's buttons** (the game's button set 1DC6h; rectangles in game pixels; "held" = called again while the button stays pressed, every 50 timer ticks in the game, 250 ms in the prototype):

| Button | Rectangle | Action |
|---|---|---|
| Up arrow (ICONES 49) | (38,159)-(54,172) | tilt +8, held |
| Right arrow (50) | (54,168)-(72,185) | rotation -32 cells, held |
| Down arrow (51) | (38,183)-(54,199) | tilt -8, held |
| Left arrow (52) | (20,168)-(37,185) | rotation +32 cells, held |
| Hawk (53) | (36,172)-(57,182) | turn to the player's place: steps of at most 32 cells and 24 rows, a globe drawn at each (`globe_turn_to`); 140 ms a step in the prototype |
| Plate (41) | (266,171)-(285,184) | turn to the mark, then open the map there |

The arrows' pictures sit at (40,161)-(50,171), (57,173)-(67,183), (40,185)-(50,195), (23,173)-(33,183); the hawk's visible shape is centred in its rectangle.

**Over the disc** the command list shows SEE MAP OF THIS AREA. A click (without dragging) on the disc converts the point to a map position (the formula above), sets the mark there (row rounded) and opens the map.

**The small globe in the panel** is a still picture of the same globe, centred on the palace (`u = 1915h / 65536`, row -4), in the eye-shaped socket: a disc of radius 16 game pixels centred at (45.75, 178.25), darker towards its rim (`globe/mini_globe.py`).

## 3. The flat map screen

**Layout.** The view is (4,4)-(316,148), centre (160, 76). Round it, on all four sides of the 320 x 152 picture, the game's frame: four lines from outside to inside in (81,113,150), (105,138,178), (134,166,203), (166,195,227). It covers the whole picture outside the view, so nothing of the globe screen shows. Then the panel (the marble one, with its right block's slate empty), its buttons, the icons, and a caption box at the top left (DUNE MAP).

**The terrain** (the game's driver slot 29). Four screen lines to a row and four pixels to a cell of each row's own ring, about the mark's meridian:

```
row = mark.row + (y - 76) / 4                     // fractional: the lines between rows interpolate
L   = L(row)                                      // the ring, interpolated between rows
du  = (x - 160 - 2) / 4 / L                       // in turns; the pure value of a cell sits 2 pixels right
outside the ring (|du| > 1/2): the darkest map colour (narrow rings are drawn once)
colour = surface(mark.u + du, row)
```

- Each row is scaled by its own ring, not the equator's: meridians curve, and moving the mark sideways is a shear, not a translation.
- **The polar mask** (from row 86): the driver covers both sides of the view, by quarter rows from row 86, with `a` pixels each side. `a` = 1, 2, 3, 5, 8, 11, 13, 16, 19, 22, 25, 28, 32, 36, 39, 44, 48, 52, 57, 63, 68, 74, 81, 89, 97, 107, 119, 138 (interpolated). The covered part is the darkest colour, with a rim of four pixels at its inner edge in the map palette's colours 1Ch, 19h, 18h, 17h (outside to inside). The row limit below means it is only ever seen at the top or bottom edge.

**Map position to the screen** (the game's `map_to_screen`, for icons and markers):

```
du = ((u - mark.u + 1/2) mod 1) - 1/2              // the signed difference, in turns
x  = 160 + du * L(row) * 4
y  = 76 + (row - mark.row) * 4
```

The game rounds x down to a whole game pixel. In HD keep it exact, or the icons move by steps while the terrain slides smoothly. **Screen to map** is the inverse of the terrain formula; the game's own `screen_to_map` cannot be used in map mode (it would divide by 2 in a way that faults).

**The places.**
- Every record of the location table is drawn: 70 records of 1Ch bytes from DS:0100h. Word 2 is x, word 4 is y, byte 8 the kind, byte 0Ah the flags (80h: not yet found, not drawn by the game).
- Each icon is centred on its place at its sprite's size, chosen by the kind (`location_icon`):

| Kind | Place | Icon (ONMAP) | Size |
|---|---|---|---|
| below 20h | sietch | 122 | 27 x 14 |
| 20h | palace of the Atreides | 123 | 36 x 30 |
| 21h..27h | village | 124 | 17 x 17 |
| 28h..2Fh | fortress | 125 | 17 x 15 |
| 30h and up | Harkonnen palace | 126 | 36 x 30 |

- Icons are clipped by the view's edge (they slide out, they do not vanish when their centre leaves).
- The pointer on the player's place (the game's sprite 4Ch) is drawn 13 game pixels above it; the prototype uses the Atreides hawk (ICONES 76, 22 x 10).
- Checked: the palace and the three sietches known at the start land within half a pixel of where the game draws them.

**Navigation** (the game's button set 1CCAh; held buttons repeat):

| Button | Rectangle | Action |
|---|---|---|
| Up arrow (sprite 37) | (267,162)-(284,171) | mark row -12, held |
| Right arrow (38) | (285,171)-(297,184) | mark x + 1002h (of 65536), held: a constant part of a turn, whatever the row |
| Down arrow (39) | (267,184)-(284,193) | mark row +12, held |
| Left arrow (40) | (254,171)-(266,184) | mark x - 1002h, held |
| Hawk (53) | centred between the arrows, (266,171)-(285,184) | the mark on the player's place |
| Small globe | (22,161)-(68,196) | back to the globe, centred on the mark (rotation = mark x, tilt = mark row) |
| EXIT MAPS (command) | | the same as the small globe |

- **The mark's row is kept within ±75** (the game clamps it at every redraw); x wraps freely.
- **There is no dragging, edge scrolling or click-to-recentre in the game.** The prototype adds dragging as a convenience: the mark moves by the pointer's movement, four pixels a cell of the row under it, still within ±75.
- The arrows' pictures: steel arrows of about 10 x 10 game pixels centred in each rectangle.

**Troops on the map** (prototype: `globe/troops.js`; a staged scene, not yet driven by the game's troop table).
- **The figures** are our rigged 3D models (`tripo/troops/KIND-3d/`), seen as the game's markers are: the camera, lean, size (24 game pixels tall in a cell of 20 x 20) and turn per direction tuned in `troops/walk.html` against the ONMAP troop sprites.
- **Baked, not live:** `walk.html`'s `bake(NAME, FRAMES)` renders one loop of a clip into a sheet, frames across and one row per direction, 8 pixels a game pixel (160 x 160 a cell), with a JSON (frames, directions, the loop's seconds). `globe/troop_sheets.py` copies them to `globe/web/troops/`. The game can use such sheets in place of the ONMAP troop sprites, one sheet per sprite group.
- **Sheets made:**

| Sheet | Figure, action | Frames | Loop | Directions | Replaces (ONMAP) |
|---|---|---|---|---|---|
| `collector-shovel` | spice collector, shovelling (Mixamo Digging, the shovel in both hands) | 32 | 4.83 s | 1 | 19..22 |
| `fighter-knife` | Fremen troop, knife training (Mixamo Stabbing, the crysknife) | 32 | 2.13 s | 1 | 43..53 |
| `planter-plant` | planter, planting (Mixamo Plant A Plant) | 40 | 7.57 s | 1 | 71..74 |
| `harkonnen-walk` | Harkonnen troop, walking (Tripo's walk) | 16 | 2.37 s | 4 | 80..95 |

- **Placing:** each troop's cell is centred on its map position, computed every frame with the map's exact `map_to_screen` (no rounding), so it slides with the map and the place icons. A troop working at a place stands beside it (+17, +3 game pixels, clear of the sietch's icon). The frame is `floor(((t / loop) + phase) mod 1 * frames)`, a random phase per troop.
- **Walking:** along the shorter way between two places, there and back, at about 6 game pixels a second; facing by where it goes on the screen: down-right row 0, up-right 1, up-left 2, down-left 3 (the order of the walking sprites' four directions).
- **The scene:** a collector shovelling at sietch 14 (spice), a Fremen fighter training at sietch 12 (army), a planter planting at sietch 13 (ecology), a Harkonnen soldier walking between sietches 15 and 14. A checkbox hides them.
- **For the game:** a troop's place and state are in the troop table (section 1): at a place (word 4) or moving (byte 3 bit 40h, with its own position at words 6 and 8 and its destination); the game's marker is `troop_marker` 62F2h (ICONES 36h at the troop's position when moving), its sprite group by activity from the table at DS:16B6h (`troop_sprite_list` 6770h) and the facing from the way to its place (6827h). Driving the scene from that table, and the walking clips of the three Fremen kinds, are the next steps.

## 4. The spice density window (SEE SPICE DENSITY)

A window over the flat map; the main map stays behind it, unchanged. Prototype: `globe/spice.js` (button "Spice density at my position", or the command). Engine: `loc_053f1` 53F1h opens it, `place_view_at` 541Fh draws it, `region_colours` 57E5h colours it, 58FAh closes it; driver slots 22 (cells) and 42 (colouring).

**Opening.**
- The command toggles the window. It always opens in spice mode. It is listed from the start and greyed before game phase 5.
- The window opens at its remembered place, `[11C1h]/[11C3h]`, which starts at (75,15). It is not placed by the click or the mark.
- Its own centre `[1980h]/[1982h]` is copied from the map's mark on opening. The row is clamped to ±42, so the 89 rows stay within ±86.

**Layout** (X,Y the window's place):
- **Frame:** ONMAP 141, 170 x 108.
- **Map:** (X+5, Y+7)-(X+165, Y+96), 160 x 89, centre (X+85, Y+51).
- **Legend band:** (X+6, Y+98)-(X+164, Y+106), colour F5h. It holds "SPICE DENSITY", then "-", the ramp (ONMAP 128: 16 swatches 3 x 5, colours 50h+k, every 4 pixels from X+95), then "+" (ONMAP 129).
- **Close corner:** the title strip left of X+10.

**Geometry.** One pixel a cell of each row's own ring:

```
row = centre.row + (y - (Y+51))
x   = X+85 + (u - centre.u) * L(row)        // map to window, as map_to_screen with cl = 0
```

- Per row the game copies the 160 cells starting at floor(u * L / 65536) - 80. A ring shorter than 160 is copied once, centred.
- **Polar inset** (|row| >= 70): `d` pixels each side, d = 2, 4, 6, 8, 11, 14, 16, 19, 22, 25, 28, 32, 36, 41, 47, 54, 65, 75 for rows 70..87. From the outside: d-4 pixels of colour 0, then 1Ch, 19h, 18h, 17h. Same colours as the main map's mask.

**Colouring** (the terrain is not shown; every pixel is replaced):
- **Regions:** `MAP2.HSQ` gives one region number per cell, in the same ring-by-ring layout as `MAP.HSQ`. A place's byte 10h is the region under it.
- **Each region's colour:** start with 70h for all. Then, for each place found (no flag 80h), in table order (later places win):
  - prospected (flag 40h): **50h + (byte 12h, the spice) >> 4**;
  - otherwise: 75h, or 78h with the MON option.
- **Borders:** a pixel whose right or lower neighbour is in another region is drawn 70h. So borders show only between coloured regions and the grey.

| Colour | RGB | Used for |
|---|---|---|
| 50h..5Fh | #200C00, #3C2400, #482C00, #583800, #644000, #744C00, #805800, #8C6400, #9C7000, #A88000, #B49000, #C4A000, #D0B000, #E0C000, #ECD400, #FCE800 | spice, from the least to the most |
| 70h | #383C3C | no region, unknown, borders |
| 75h | #54504C | found, not prospected |
| 78h | #502020 | the same in monochrome |

- **Checked** against a frame of the day 23 save: every region's colour matches.
- **In HD** (prototype):
  - the regions are smoothed at 8 texels a cell (`spice_assets.py`, `web/regions.png`: R the region, G a soft border line);
  - the borders are drawn thin;
  - the surface's relief shows faintly through the flat colours (an option, not in the game).

**Markers.**
- **Places:** every found place, centred, by kind (ICONES 3Ah + class):
  - sietch 8 x 5;
  - Atreides palace 13 x 13;
  - village 7 x 5;
  - fortress 10 x 10;
  - Harkonnen palace 12 x 12.
  - A sietch farther than `[1176h]` gets 3Fh. The limit's value and the point it is measured from are not read yet; the prototype uses 53 pixels from the player, which fits day 23.
- **Player:** the pointer (ICONES 4Ch, 22 x 10), x-13, its bottom on the player's place.
- **View rectangle:** a dotted one (colour FBh, pattern 5555h) of 80 x 40 round the centre, clamped to the map. It is about the main map's view.

**Interaction.**
- **Panel arrows,** while the window is open: they move the window's centre by the map's steps (±1002h of x, ±12 rows), not the map.
- **Title strip:** drag it to move the window. The outline is kept within x 5..144, y 5..28; the window reopens there.
- **Place in the map:** hovering within 9 pixels writes the place's type and name in the legend band. A click closes the window and centres the map on the place.
- **Legend band:** a click switches between spice density and troop occupation ([4722h] ^= FFh, 5970h; see below).
- **Hovering the ramp:** an XOR box marks the swatch under the cursor.
- **Prototype only:** dragging inside the map pans the window's centre (not in the game).

**Troop occupation** (the other mode of the same window). The geometry, borders, markers and interaction stay the same; only the colours and the legend change.

- **Counting** (`region_colours` 583Fh, with the callback 5728h through `loc_06639`): for each place found, count your troops there:
  - the place's own chain: the first troop's id is the place's byte 9, each troop's next id is its byte 1;
  - then every troop moving (byte 3 bit 40h) whose place (word 4, or word 0Ch when byte 3 & 3 = 3) is this place.
  - A troop counts when it is yours (byte 3 < 80h) and its job (byte 3 & 0Fh) is not 2: jobs 0..3 add to **BL** (spice), 4..7 to **CH** (army), 8..15 to **CL** (ecology).
- **The pattern byte** of the region: index = (BL > 0) + 2 (CH > 0) + 4 (CL > 0), then (586Eh):

| Index | Byte |
|---|---|
| 0 | 00h |
| 1 | 55h |
| 2 | AAh |
| 3 | 66h if BL = CH, 9Ah if BL < CH, else 65h |
| 4 | FFh |
| 5 | 77h if BL = CL, DFh if BL < CL, else 75h |
| 6 | BBh if CH = CL, EFh if CH < CL, else BAh |
| 7 | BL < CH: DEh if CH < CL, else 9Bh; BL >= CH: 7Bh if BL < CL, else 67h |

  Regions without a found place keep 70h.
- **Drawing** (driver 24E9h): each line k of the window (from its top) takes one pair of bits of the byte: k mod 4 = 0 bits 1-0, 1 bits 7-6, 2 bits 5-4, 3 bits 3-2. The pair p gives colour 71h + p:

| Pair | Colour | RGB | Meaning |
|---|---|---|---|
| 0 | 71h | #383438 | no troops |
| 1 | 72h | #E8A000 | spice |
| 2 | 73h | #E82800 | army |
| 3 | 74h | #00B000 | ecology |

  So a single job fills its region in one colour, and mixed jobs give stripes one line high in proportion to the counts (9Ah, more army than spice: three army lines to one spice line). 70h stays 70h; borders as in spice mode.
- **Legend:** the game writes only "TROOP OCCUPATION". The prototype adds a key of the three colours (not in the game).
- **Hovering a place:** the game shows its troops by job in the legend band (ONMAP sprites 82h, 83h, 84h, 5 x 7, each with ":" and a digit, at X+71h + 12h·k), or "no troops". The prototype writes "spice a, army b, ecology c".
- **Checked:** on the day 23 save the counts add up to the 15 rallied troops the game's map caption gives, and the window matches the game's frame region by region (places 24 and 25, one spice troop each, are the only coloured regions in that view).

**The HD drawing of the window** (prototype, `globe/spice.js`):
- Its own WebGL canvas at 8 pixels a game pixel, inside a window DOM element positioned in game coordinates, above the map's icons, so the window covers them as the game's does.
- **Map:** the smooth region texture (nearest sampling: R the region, G the soft border), a 256 x 1 colour table built from the places (spice mode: RGB; troop mode: the pattern byte in R and 255 in G for a found place), the ring table as a 128 x 1 texture, the HD surface for the relief.
- **Borders:** the G channel mixes towards the grey of 70h; the relief factor is applied to both, so borders vanish inside the grey as in the game.
- **Relief option:** factor clamp(1 + 1.1 (l - 0.78), 0.72, 1.25) of the surface's luminance l (0.78 is about its mean), so the colours keep their mean.
- **Stripes** are one game pixel high (8 HD pixels), counted from the window's top as the driver does.
- **Frame and legend:** painted on canvases in blue steel (bevel, rivets, the close cross, the legend). The legend is on its own canvas, hidden while hovering a place so the hint can take its place. This is drawn in code; no generated art yet.
- **Markers:** our HD place icons (ONMAP 122..126 at the window's small sizes), the far sietches darkened by a filter, the hawk as the pointer.
- **Places' state:** the page uses either the start (from the executable) or the day 23 save (`state-day23.json`: per place flags, spice and the troop counts).

## 5. The ornithopter, map side (TAKE AN ORNITHOPTER)

Prototype: `tripo/flight/flight.html` (with `flight/terrain.js`, the 1:1 map renderer). References: frames captured from the day 10 save (`flight/frames/`). The dunes' video of the flight is not covered (a plain sky stands in). Engine addresses are seg000 of `DNCDPRG.lst`.

**Map position and the 1:1 maps.** A position is x (16 bits of a turn) and a row y. Both maps here are `map_draw_zoomed_globe` with one pixel a cell: `sx = cx + ((x - mark.x) * L(y)) >> 16`, `sy = cy + (y - mark.y)`. `bp(y) = [4880h + 2|y|]` = round(65536 / L(y)) is the x of one cell. Polar inset as in section 4.

**The destination map** (phrase A7h; handler 42E9h, set-up `loc_0430b`):
- **Window:** rect 149Ch = (81,45)-(241,134), 160 x 89, centre (161,89) = the player's position; the centre row clamped to ±42.
- **Cockpit:** `[473Eh]=1`: sky, cockpit sprites (resource 24h, list 14B4h), the overlay sprite 2 at (79,45) over the map. The green grid is not drawn by code: it is presumably part of that overlay. The prototype draws it as fixed lines at x 121, 160.5, 200 and y 66, 89, 110.5.
- **HD cockpit:** generated with Banana Pro from the game's frame (`flight/cockpit.png`). Its screen came out at (80,38.5)-(240,117.6); the page stretches the picture vertically about it to fit the game's window (35..135).
- **Scrolling:** the destination map uses the map screen's own button set (1CCAh), so the cockpit's four arrows and the hawk act as on the map:
  - its centre is the map's mark (`[197Ch]` x, `[197Eh]` row), set to the player when the map opens (`loc_05b5d`);
  - each arrow adds its entry of the step table 145Eh..146Ch to the mark (`ui_click_map_buttons` 8831h): 1002h of a turn sideways, 12 rows up or down; then the map is redrawn through `[46EDh]` = 4377h; a held arrow repeats;
  - the hawk sets the mark back to the player's position (`loc_05b1e`);
  - the row is kept within ±42 when the map is drawn.
  - The prototype adds the keyboard's arrows and dragging (one cell per game pixel; a drag does not choose a destination); neither is in the game.
  - Hover, click and the player's marker are all relative to that centre, not to the player.
- **Title:** string 57h ("SELECT DESTINATION ON MAP"), typed one glyph every 18h ticks (spaces at once), at (85,38), small font.
- **Places:** every found place in view, centred, ICONES 3Ah + kind class (as section 4). A sietch farther than `REACH [1176h]` gets 3Fh. The distance is Chebyshev in cells: max(|Δx| / bp(y), |Δy|). The prototype dims those beyond 50 (the value is not read).
- **Player:** ICONES 4Ch at (sx-13, sy-height), blinking (frame task 44ABh, every 12Ch ticks).
- **Hover** (`loc_04586`):
  - a found place with screen |dx|+|dy| < 9: the line shows its type and name;
  - else, on one of eight narrow rays from the marker ((a+3) & 1Fh < 6, about ±4°; `a = dir(dx, dy)` from the marker's point (left+0Bh, bottom)): "heading" + the compass point (phrases A4h, DAh+k);
  - else phrase A4h alone.
- **Click** (`loc_0450e`): a place, or any point in the view. No range check. Your own place is ignored on the ground. The line blinks 9 times (14h ticks off, 0Ah on), then the flight starts or turns (`loc_04944`):
  - a place: target = it, homing (the heading recomputed at every step);
  - any other point: the heading to that point, kept (free flight), target = the last place.
- **Direction** (`loc_0514e`): 256 a turn, 0 north, 40h east, y down is south:
  - if |dx| >= |dy|: a = 20h·dy/dx + (dx >= 0 ? 40h : C0h);
  - else: a = -(20h·dx/dy - (dy >= 0 ? 80h : 0)).
  - Between two map points (`loc_05133`): dx = (x2 - x1)/2 (16-bit, the short way), dy = (y2 - y1)/2 · 256.
- **Cancel** (phrase A3h): back to the ground if no flight was started, else back to the flight.

**The flight** (`sub_4f09`, one step every 300h ticks of `[CE7Ah]`: about 2.5 s in the captured frames; the prototype offers 1x, 4x, 16x):
- **Step** (`heading_step` 5206h):
  - With a free heading (or no target) above row 4Dh, a heading pointing to the pole is turned east or west; when homing, the heading is the direction to the target.
  - Step vector (`loc_05198`): 20h along the heading's main axis, the offset within the quadrant across.
  - `xs = bp·sx/32`, `ys = bp·sy/32`, `f = |ys| + frac`; if f >= 200h: xs = xs·256/f, f = 100h; frac = f & FFh; y += sign(ys)·(f >> 8); x += xs.
  - Past a pole: heading + 80h, x + 8000h.
- **Clock:** every 16th step the game's clock advances one slot (16 a day). Not in the prototype.
- **Arrival:** in the target's map cell. The player is at that place and its room is shown.
- **The small map:** rect 148Ah = (204,4)-(316,60), 112 x 56, centre (260,31), frame of four outlines (FCh..F6h) round it.
  - It is centred on the mark: the take-off point, then the ornithopter's position whenever its point leaves 214..305 x 10..53.
  - It draws the found places, the target (sprite 2Eh), the trail (sprite 2Fh, a ring buffer of 276 positions, emptied at take-off, kept by CHANGE DESTINATION) and the ornithopter (sprite 30h).
  - Checked: in the captured frames the map stays put and the trail grows towards the target.
- **Commands:**
  - Homing: SKIP TO DESTINATION (4FFBh: up to 200 steps at once, stopping on an event, then landing at the target in any case) and CHANGE DESTINATION (497Ah: the destination map again, the flight held, the trail kept).
  - Free flight: a return to the last place (ACh → 50A5h), from phase 32h the nearest found place (AAh → 50C4h, not in the prototype), and CHANGE DESTINATION.
  - Steering buttons (set 1D72h, free flight): (258,172) heading -4, (270,170) one step, (283,172) heading +4.
- **Discovery** (`loc_040f9`, with a party):
  - An unknown place (flag 80h, its phase reached) within ±4 cells of the position, with `rel = dir(pos→place) - heading + 60h < C0h` (ahead, within about 135°), is found: flag 80h cleared.
  - A phrase says it is on the left (rel < 60h) or the right; the flight retargets to it and asks (list 1F92h).
  - The prototype finds and retargets without asking.
- **Not in the prototype:**
  - the spice watch over cells with bits 30h (`loc_04182`: the Skip command disabled, then a communicator call);
  - the ornithopters' count at the places (byte 15h);
  - the clock;
  - the phrase texts (the prototype writes its own: "Sietch: place 13", "HEADING EAST");
  - the landing's room;
  - the nearest-place command;
  - the dunes' videos (chosen from the terrain ahead, `loc_04e8e`).

## 6. Assets

All game-derived, made by the scripts in `tripo/globe/` (outside this repo; the pictures are in no repo):

| Asset | File | Made by |
|---|---|---|
| The planet's surface | `globe/surface-final.png` (4096 x 2048) | `globe_data.py`, `globe_surface.py` |
| The globe screen's plate | `globe/plate-people.png` | recipe `plate`, `plate_people`, `globe_plate.py` |
| The panel in marble | `globe/web/panel-marble.png` (2560 x 592, rows 126..200) | recipe `panel_marble`, `globe_assets.py`, `mini_globe.py` |
| The place icons | `globe/icons/ONMAP-122..126.png` (40 pixels to a game pixel) | recipe `icons`, `globe_icons.py` |
| The arrows and the hawk | `hud/pieces/ICONES-037..040, 049..054, 076.png` | `mini_arrows.py`, `mini_side_arrows.py`, `hud_insignia.py`, `globe_icons.py` |
| The regions | `globe/regions.png` (398 x 197), `globe/web/regions.png` (smooth, 3184 x 1576) | `globe_regions.py`, `spice_assets.py` |
| The location table | `globe/locations.json` (with each place's region from MAP2 and its starting spice) | read out of `DNCDPRG.EXE` (file offset F7B0h) |
| The small globe | in `globe/web/panel-marble.png` | `mini_globe.py`, called by `globe_assets.py` |
| The page's data | `globe/web/data.json`: the ramp, the fit, the rings, the polar mask, and per place u, row, kind, flags, region, spice and the troops at the start | `globe_assets.py` (with `troop_counts.py`) |
| The troop sheets | `globe/web/troops/NAME.png` and `.json` | `troops/walk.html` `bake()`, then `troop_sheets.py` |
| The HD cockpit | `flight/cockpit.png` (5504 x 3072), `flight/web/cockpit.jpg` | Banana Pro from the game's frame (`flight/cockpit-input.png`, its screen blacked out) |
| The flight's panel | `flight/web/panel.png` | the HUD master (`hud/base.png`) with the globe panel's alpha and head |
| The day 23 state | `globe/web/state-day23.json`: per place [flags, spice, spice troops, army, ecology] | `spice_assets.py`, then `troop_counts.py` |

Order to rebuild: `globe_regions.py DUNE.DAT`, `spice_assets.py`, `globe_assets.py`, `troop_counts.py` (the last two read the executable; the save's place table sits at decoded offset 17695 of `day23-DUNE37S1.SAV`).

## 7. Hooking it into the game

- **Recognise the screens.**
  - The globe is drawn by driver slots 23 and 24 (a row a call); the flat map by slot 29; the small maps by slot 22.
  - The HD layer decodes none of them today: they come out low-resolution (`crates/gfx/src/ops.rs`: `Other`).
  - The cleanest hook is a mark at `setup_globe_draw` / `map_func_gfx` and at `map_draw_zoomed_globe` (B6C3h), as the rooms and heads have. The mark reads the state the HD drawing needs: rotation `[494Ch]`, tilt `[2460h]`, the mark `[197Ch]`/`[197Eh]`, the mode `[46EBh]`, the results' colour mode `[DD02h]`. For the spice window also its place `[4710h]/[4712h]`, its centre `[1980h]/[1982h]` and its mode `[4722h]`; the colours come from the place and troop tables above, so the HD layer can rebuild the table itself rather than read the driver's.
- **Draw, don't upscale.**
  - At the mark, draw the disc or the map from the HD surface with the formulas above, at the output resolution, and skip the driver's pixels for that area.
  - The places, markers and troops the game then draws with sprites (`ONMAP`) are matched by their sprite as today, and placed at their exact positions (sub-pixel) rather than the game's rounded ones.
- **What the game changes over time** is not in the HD surface: the state bits of the cells (vegetation, water), the results view's tints, the place view's region colours from `MAP2.HSQ`. Draw them from the live cells, in the same map coordinates, over the surface.
- **Not covered by the prototype yet:**
  - the ornithopter's details listed at the end of section 5, and its dunes' videos;
  - the spice window's farther sietches' limit `[1176h]`, the place names in its hover line, and the legend's swatch highlight;
  - troops driven by the game's troop table (the prototype shows a staged scene), the other kinds' walks and the other activities (poking, idle, prisoner, the unknown troop's outline);
  - the side panels' sliding when the results open;
  - the results view's bars and colours;
  - the game's exact timings (the tick length of the globe's rotation and of held buttons).
