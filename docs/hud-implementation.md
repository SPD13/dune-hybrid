# The HUD (the bottom panel), the book's page turn and the palace plan: how to implement them in the game

This is the specification for drawing the game's panel in HD: the bottom panel of every screen (the frieze), its states (side blocks, button sets, the head, the people slots, the sun, the moon and the day), the open book's page turn, and the palace plan. It is written from the engine's own code (the decompile worktree's `hand/frame.rs`, `hand/globe.rs`, `hand/screens.rs`, `hand/panels.rs`, `hand/ui.rs`, `hand/gfx.rs`, `hand/tails.rs`, `hand/scenes.rs`; `DNCDPRG.lst` in Cryogenic's `doc/`; the tables in `DNCDPRG.EXE`, where DS:0 is file offset F6B0h) and from the HD pilot and its prototypes (`Cryogenic-Web/portrait-proto/studio/tripo/hud/`, in the private repo `SPD13/dune-talking-heads`). How the HD art was made is in [talking-heads-3d.md](talking-heads-3d.md) (the HUD pilot); the settings panel has its own document, [hud-mixer-implementation.md](hud-mixer-implementation.md).

**Principle.** The game's state and logic stay as they are: which side set and button set are up, which buttons are live, the head's step, who is in the slots, the time, the day, the book's page. Only the drawing changes. Every coordinate is in the game's 320 x 200 pixels (the view above the panel is 320 x 152; the panel is rows 152 to 199), so the HD drawing can sit on the game's state and follow it.

**Scale.** The HD panel is one master picture of the whole screen, 5224 x 3264 (`hud/base.png`): K = 5224 / 320 = 16.325 pixels to a game pixel. Every HD piece is cut from it at `(x * K, y * K)`, so pieces join without seams.

## 1. The frieze: the panel's UI elements

`redraw_frieze_screen` (B23Fh) clears the screen and draws the 12h UI elements of the list at DS:1AE6h, then `sub_d397` and `sub_b039`. A UI element is 0Eh bytes: x0, y0, x1, y1 (words), flags (word +8), sprite (word +0Ah; FFFFh none; from `ICONES` plus the element's palette offset), handler (word +0Ch).

| # | Element | Rectangle | Sprite | What |
|---|---|---|---|---|
| 0 | 1AE6h | (22,161)-(68,196) | from the side set | the globe icon's place (sprite 0Dh on the globe and map screens) |
| 1 | 1AF4h | (0,152) | from the side set: 0, 6 or 9 | the left block, 92 x 48 |
| 2 | 1B02h | (228,152)-(300,198) | 3 | the right block, 92 x 48 (the decompile calls it the date panel) |
| 3 | 1B10h | (24,155)-(69,176) | none | the closed book's click area (flags 80h in the closed book's set; handler AED6h) |
| 4 | 1B1Eh | (92,152)-(229,159) | 0Eh (14) | the strip above the commands, 136 x 7 |
| 5, 6 | 1B2Ch, 1B3Ah | (2,154), (317,154) | 0Ch (12) | a thin strip at each outer edge, 1 x 5 |
| 7-11 | 1B48h ... 1B80h | (92, 159 + 8k)-(228, 167 + 8k) | 1Bh (27) | the five command rows, one 136 x 9 sprite drawn five times; handlers D443h, D43Eh, D439h, D434h, D42Fh |
| 12-17 | 1B8Eh ... | the button set (section 3) | | copied from the set in use |

**The head** stands over the room: sprite 10h + step (20 x 23) at (150,137), and its shoulders 0Fh (68 x 4) at (126,148) (section 4). The head panel's rectangle is 1E6Eh: (150,137)-(170,160).

**Colour is not fixed.** Every draw adds the element's palette offset (an arrow that cannot be used is the same sprite with another offset); colours F0h to FFh are rewritten with the time of day and faded; opening `ONMAP`, `ATTACK` or `GENERIC` rewrites part of that range, which is why the map screen's panel is the same sprites in blue. The HD panel has one set of colours (the room screen's): the map screen's blue needs a tint or a second master (open point).

## 2. The side sets (the left block and its companions)

`ui_set_and_draw_frieze_sides` (D795h) copies four (flags, sprite) pairs into elements 0 to 3 (their words +8 and +0Ah) and draws elements 0 to 2. The four sets (4 bytes a side):

| Set | Table | Entry | Element 0 (globe icon) | Element 1 (left block) | Element 2 (right block) | Element 3 (book area) |
|---|---|---|---|---|---|---|
| closed book (room screens) | 1C36h | D75Ah | none | 0 | 3 | flags 80h (live) |
| globe | 1C46h | D7B2h | 0Dh, flags 40h | 6 | 3 | none |
| open book | 1C56h | D7ADh | none | 9 | 3 | none |
| map | 1C66h | D792h | 0Dh, flags C0h | 6 | 3 | none |

The closed book's entry (D75Ah) also draws the date (section 6) and the people slots (section 5).

**The nudge** (`nudge_panel`, D23Dh): when the sets change, the sprite words of elements 1 and 2 are stepped by 1 towards a multiple of 3, drawn (2 elements, then the date), a pause of 0Ah, stepped and drawn again. The in-between sprites are 1, 2 (after 0), 4, 5 (after 3), 7, 8 (after 6). With the HD panel drawn as layers these are not needed: a transition of our own (a slide or a crossfade over the same two steps) when the set changes.

## 3. The button sets (the right block)

`set_buttons` (D72Bh) copies 6 UI elements into 1B8Eh (and the rectangle at 2458h) and draws them. `set_buttons_for_screen` (D717h) picks: 1CCAh when the map view has hot spots (`[46EBh]`), else 1D72h when the travel screen's bits 3 are set (`[11C9h]`), else 1C76h. The book sets 1E1Ah (`ui.rs`, AF2Fh).

| Set | Elements (rectangle, sprite) |
|---|---|
| 1C76h, default | the frame 21h (33) at (255,162)-(295,192); the arrows 1Dh-20h (29-32): up (269,162)-(279,172), right (284,172)-(294,182), down (269,181)-(279,191), left (255,172)-(265,182); the palace plan button 24h (36) at (269,173)-(280,181) |
| 1CCAh, map | 29h (41) at (266,171)-(285,184); the arrows 25h-28h (37-40) at (267,162)-(284,171), (285,171)-(297,184), (267,184)-(284,193), (254,171)-(266,184), flags 4080h; the hawk 35h (53) at (266,171)-(284,183) |
| 1D72h, travel | 2Ah, 2Bh, 2Ch (42-44) at (258,172)-(266,182), (270,170)-(279,182), (283,172)-(291,182); 29h (41) at (266,171)-(285,184); 31h (49) at (38,159)-(54,172) |
| 1E1Ah, open book | no sprites: the left page (21,161)-(42,186), handler AFC7h; the right page (43,161)-(63,186), handler AFB5h; the left block (0,154)-(92,200), handler B18Bh (closes the book); the frame 21h at (255,162)-(295,192) |

A button that cannot be used is drawn with another palette offset; a live one normally. In the HD panel the base shows every arrow closed and no button, and an overlay with alpha is drawn for each live arrow and for the palace plan button (`ICONES-029` to `032`, `036`).

## 4. The head

- **The step** `[0E8h]`, 0 (down) to 10 (up): sprite 10h + step at (150,137).
- **Up** (`ui_head_animate_up`, 17E6h): one step every 8 ticks to 0Ah, unless `[11C9h]`; when the player gets the hand (after a scene refresh outside a place, back from the map, after a dialogue). A draw trace shows the sprites 16, 17, 17, 16, 20, 21 ... 26, 0.04 s apart (18 and 19 are the pictures of 17 and 16): the head dips one pixel before it rises, 7 or 8 pixels in all.
- **Down** (`ui_head_animate_down`, 181Eh): one step (`sub_17be`, the head panel redrawn) every 8 ticks to 0, when a talk or a film takes over.
- **The blink** (`head_blink`, 1843h): with a head shown, frame 9, a pause of 8, frame 8: a dip of two pixels, not closed eyes.
- **HD:** three flat layers in place of the 11 sprites (`hud_head.py`): the collar and stone hump with the head taken out (`ICONES-015`, drawn first, alpha over the room), the head (`head.png`), moved down by the step's offset, then the collar's front rim (`collar-front.png`) over it. The offsets per step are the recipe's `head.rows`: 7, 8, 8, 7, 6, 5, 4, 3, 2, 1, 0 game pixels below the top for steps 0 to 10. For a smoother move, slide from 7 to 0 with easing over the same 0.4 s and leave the dip out. For an engine that still draws sprite 16 + step, `hud_cut.py` also composes the 11 sprites (`ICONES-016` to `026`, 20 x 23, alpha).

## 5. The people slots

- **The slots:** elements 1C0Ch (35,182)-(56,196) and 1C1Ah (58,182)-(79,196), flags 80h; who is in them: `[1152h]`, `[1153h]` (a character 0 to 8, FFh nobody); a filled slot is a hot spot (`dialogue_click`).
- **Drawing** (`draw_gauges`, D763h): both slots as sprite 40h (64, empty), then 41h + the character (65 to 73: Leto, Jessica, Thufir, Duncan, Gurney, Stilgar, Kynes, Chani, Harah).
- **The blink** (`blink_gauges`, D7B7h): every 64 ticks of `[0CE7Ah]`, a counter per side `[2222h]` steps; on odd steps that side's slot is drawn as FFh (empty) and the real value comes back; not while `[1AFEh]` is set.
- **HD:** `ICONES-064` (the empty slot, cut from the master) and `ICONES-065` to `073` (each character's head from its HD portrait, set into the recess: `hud_faces.py`). The prototype's blink step is 0.32 s, an estimate (64 ticks).

## 6. The sun, the moon and the day

`ui_draw_date_and_time_indicator` (1A34h), on the screen, only when `[1AFEh]` (the left block's sprite word) is 0, so only with the closed book's set:

- **The slot** = game time & 0Fh (16 a day). The table at DS:1E7Eh, 8 bytes a slot: the sun's top-left corner (x, y), then the moon's; x = 0: not drawn. The sun (sprite 4Ah, 10 x 8) then the moon (4Bh, 3 x 4), each by `draw_moon_and_sun` (1A9Bh), clipped to the rectangle at 1EFEh: (6,181)-(30,190).

| Slot | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sun | 6,187 | 6,186 | 6,185 | 7,183 | 9,182 | 10,181 | 13,181 | 16,181 | 18,182 | 20,183 | 20,185 | 20,186 | 20,187 | - | - | - |
| moon | 25,186 | 26,188 | - | - | - | - | - | - | - | - | - | 8,188 | 9,186 | 12,183 | 17,182 | 23,183 |

- **The day:** `get_ingame_day`, reduced to the day of the year (less 365 while 365 or more), plus 1; written in the small font, colour F1FAh, by `draw_number_at_pos` at x 0Bh, 9 or 7 (3, 2 or 1 digits: right-aligned) and y BEh; then a space. In a frame of the game the "1" of day 1 is at x 17, rows 191 to 195, in the middle of the alcove.
- **HD** (`hud_sky.py`, prototype `hud/sky.html`): the sun and the moon are drawn in code (`ICONES-074`, a white-gold disc in a warm glow with a level glint and four short rays; `ICONES-075`, a thin pale crescent lit from the right). The HD alcove is one tall arch, rounder than the game's rectangle, so the HD keeps the game's timing (which slots each is up) but not its positions: each body's centre follows a half ellipse inset in the arch, centre x 19.29, standing on the horizon y 191, half-axes 8.12 across and 7.37 up (2.6 game pixels inside the arch's opening); on the horizon at its first and last slot (sun 0 and 12, moon 11 and 1), at the top half way, gliding between slots. They are clipped to the arch's dark opening (`hud/sky-mask.png`) with a soft horizon over the last game pixel. The day number is written centred on the arch's middle (x 19.3), standing on row 196, in the HD font (the mock-ups use Garamond Bold as a stand-in).

## 7. The book's page turn

- **When:** a click on the open book's left page (21,161)-(42,186) calls AFC7h (the decompile's next page; nothing when `[0C6h]` bit 1 forbids it; with bit 2, two pages on and the music back); a click on the right page (43,161)-(63,186) calls AFB5h. Both find the page (`loc_0b150`, CX 1, the page `[11BFh]`, BX `[2406h]`) and, when there is one, draw the turn; past the end, the book's frame is drawn instead (`loc_009f5`).
- **The direction** is DX: AFC7h sets DX = FFFEh (negative), AFB5h DX = 2 (or 0 with `[0C6h]` bit 2); the end of the book uses DX 0, a step back DH = FFh.
- **The drawing** (`ui_draw_book_turning_page`, B1AFh), on the screen, the `ICONES` sheet loaded: at (1Bh, 9Eh) = (27,158), with DX 0 or more, sprite 0Ah (10, the page lifted over the right page) then 0Bh (11, the page over the left page); with DX negative, 11 then 10. Each sprite is 31 x 29, followed by `wait_a_bit` 0Ah. Then the left block (element 1AF4h, the open book, sprite 9) is drawn again.
- So DX 0 or more is the right page turning over to the left; DX negative is the left page turning over to the right.

**HD** (`hud_book.py`, prototype `hud/book.html`): no sprites. The page that turns is the HD book's own page.

- **The pages** are cut out of the open book's piece (`ICONES-009`, the left block from (0,146), 1502 x 881: the open book stands 6 rows above the panel):
  - each page's top surface found by its light parchment, the handwriting filled in, the stack's edges dropped;
  - grown into its shaded bottom (brightness over 100, downward and in its bottom third a little sideways);
  - filled out to the crease;
  - saved as `hud/book-page-left.png` and `book-page-right.png` with alpha, their boxes in `hud/book.json` (corners on even master pixels).
- **The crease** is found row by row as the darkest point of the gutter, in two passes. It is fitted by a quadratic, `x = a y^2 + b y + c` in master pixels (`book.json` `crease`): slanted, from x 806 at the top of the pages to 721 at the bottom.
- **The turn** at progress p (0 to 1), direction dir (+1: the right page to the left; -1: the left page to the right). Each row y of the turning page hinges on the crease at that row.
  - `ease(p)` = 2p² below 0.5, else 1 - 2(1 - p)²; `th0 = π · ease(p)`.
  - Across the page (s from 0 at the crease to 1 at the outer edge), the angle `th(s) = th0 + 1.2 · sin(πp) · s²`, clamped to [0, π] (the leading edge first). Integrated over 30 columns: `X(s) = Σ cos th / 30`, `Z(s) = Σ sin th / 30`.
  - Over 12 rows (v from 0 at the top to 1 at the bottom), each row mixes the two pages' rows by `f = (1 - cos th0) / 2`. Its width w is the distance from the crease to the page's outer side, mixed the same way.
  - A point lies on the screen at `x = crease(y) + dir · w · X - dir · 0.38 · w · Z`, `y = row - 0.45 · w · Z · (1 - v)`. The lean (0.38) makes the standing page show its face both ways, as the game's frames do. The rise tapers to nothing at the bottom row, so the page's foot stays on the book.
  - Where `th > π/2` a cell shows the back, which is the opposite page's picture, so the page lands exactly as that page.
  - Each cell is darkened by `0.3 · sin th` (+0.05 on the back) with the page's own alpha in dark brown (74, 35, 12).
  - A soft shadow falls on the pages under it: a gradient from the crease outwards on the side the page is over, strength `0.4 · sin th0`, only on the pages.
  - The cells are drawn as affine-mapped triangles, each grown by about a pixel against seams.
- **Timing:** the game shows two frames of 10 ticks of `wait_a_bit`. The engine's tick is 200 a second (checked on the intro's videos, [talking-heads-3d.md](talking-heads-3d.md) section 12; the head's 8 ticks are its 0.04 s), so the game's turn takes about 0.1 s, if `wait_a_bit` counts the same ticks. The prototype's default is 0.7 s, adjustable: a smooth HD turn reads better slower than the game's two flashes.

## 8. The palace plan

The palace plan button (24h, element 1CBCh of the default set, handler 18EEh) is live only in the palace: `[1BDCh]` and `[1CC4h]` are 80h at place 100h, else 20h.

- **Opening** (`ui_draw_palace_plan`, 18EEh): when the plan is already up (the screen's handler list is 2012h), a click leaves it (`leave_and_fade`). Else, in the palace (`[4]` 20xxh, but not room 1):
  1. the screen's handlers 1ABAh (a click anywhere: `leave_and_fade`, D2E2h);
  2. the screen saved;
  3. the rectangle `[143Ch]` (160,0)-(320,116) filled (far `[38DDh]`, colour F1h) and outlined at `[1444h]` (164,4)-(316,112) (colour F7h);
  4. resource 21h (`PALPLAN`) and the sprite list 120Bh;
  5. the people (`people_panel`, 1948h);
  6. the rectangle faded in (transition 10h).
  The command list then shows "Done".
- **The sprite list 120Bh** (sprite, x, y; FFFFh ends):
  - the plan, sprite 0 (114 x 92), at (182,12);
  - the lower wing's closed doorways: sprite 3 (2 x 7) at (266,65), 4 (19 x 13) at (238,65), 5 (19 x 23) at (193,65).
- **The story ends the list early,** so the doorways open:
  - `on_phase_0c` writes FFFFh at `[121Dh]`: sprite 5 gone;
  - `on_phase_1c` at `[1217h]`: 4 and 5 gone;
  - `on_phase_54` at `[1211h]`: all three gone.
- **The people** (`people_panel`): counted per room for each of the 16 characters (FD8h, 10h bytes) at the player's place (`[4]` high byte against their byte 3) and known (`loc_0127c` with byte 0Eh), at index byte 0 - 1; those in the party (bit 40h of byte 0Fh) at +0Ch. The player's room is index `[4]` low byte + 17h (if 0Ch or less).
- **The 11 rooms of table 1426h** (byte offsets from the plan's place (182,12)): 0 (29,65), 1 (85,49), 2 (29,33), 3 (57,17), 4 (57,65), 5 (29,49), 6 (1,49), 7 (29,1), 8 (29,17), 9 (57,49), 10 (1,65). For room (x, y):
  - the first count's marks (sprite 2, 3 x 3) from (x + 3, y + 2), up to 5, 4 apart (`draw_marks`, 19DFh);
  - the party's 7 rows lower;
  - the player's room: sprite 1 (5 x 5, red) at (x + 12, y + 5).
- **Checked against the running game** (`dune-run`, the Day 1 09:00 save, the button clicked at (274,177); `hud/palplan/frame-game.png`): the panel, the plan with the three doorways closed, two people's marks at (214,31) and (214,47), the player's dot at (223,34) in room 8, "Done" in the command list.
- **Not used here:** sprite 6 (the word DUNE) and 7 (a 130 x 74 frame: the dialogue's inset map, list 22E4h at (168,23), the map inside (176,32)-(288,88), when a character says where someone is).

**HD** (`hud/palplan_ref.py`, `hud/palplan-recipes.json`, `hud/palplan_gen.py`, `hud/palplan_cut.py`; prototype `hud/palplan.html`):

- **The tablet:** asked to redo the whole panel, Banana Pro drew another palace (other rooms, swinging doors), so it makes only the tablet: the master (`master.png`) and an edit with the plan taken off (`tablet.png`: a carved sandstone and bronze frame round plain dark slate). That is two calls through Google, about $0.48.
- **The plan is drawn in code** from the game's sprite, so every wall is the game's. The walls (the sprite's (190, 97, 48) pixels) and the arrow ((227, 170, 105)) become raised copper inlaid in the slate:
  - the pixels enlarged and rounded (blur K × 0.28, thinned a little);
  - a height from their blur (K × 0.22) lit from the upper left (-0.55, -0.65, 0.52) with a specular;
  - a soft shadow on the slate (K × 0.35, offset +0.25, +0.35).
  The word OUT is set again in Garamond Bold in the same copper, on its pixel letters' box.
- **The doorway walls** (`wall-3.png`, `wall-4.png`, `wall-5.png`) are drawn together with the plan's walls and kept only within two game pixels of the wall, so they join without a seam. They go over the base (`base.png`, the doorways open) at `pieces.json`'s places.
- **The marks:** a person is a bronze stud with a cross cut in it (`mark.png`, 3.4 game pixels across, centred on the game's 3 x 3 sprite); the player is a red gem (`player.png`, 5.2 across, centred on the 5 x 5 sprite).
- **In the game:** mark `ui_draw_palace_plan` (18EEh) and `people_panel` (1948h). Draw the tablet at (160,0), 160 x 116, then the doorway walls still in the list, then the marks from the same counts; fade the rectangle in. On leaving, the game redraws what was under it.

## 9. The colours by place and hour

The game's panel has no colours of its own: its sprites (`ICONES`, `PALPLAN`, `MIXR`) are drawn with the palette's last 16 entries, 240-255, which the place leaves there (reading in `tripo/recolour_views.py`):

- **Indoors:** the room's sheet sets them: a brown-gold ramp in the palace, a red-gold one in a sietch. They are the same at every hour.
- **Outdoors:** the sky palette of the hour sets them (`set_sky_palette`, 388Dh): sub-resource N of `SKYDN.HSQ`, with N from the table at DS:2280h by the slot of the day (game time & 0Fh), plus 4 x the day's palette group (0 to 7).

| Slots | 0-1 | 2-10 | 11-12 | 13-15 |
|---|---|---|---|---|
| SKYDN | 8 dawn | 9 day | 0Ah dusk | 0Bh night |

- **What turns:** the stone, the book, the alcove, the strip, the command rows, the head and its collar, the arrows and their frame, the empty slots, the sun and the moon.
- **What keeps its colours** (other indices): the palace plan button (36), the map buttons (37 to 41), the hawks (53, 76), the globe's arrow (54) and the faces (65 to 73).

**HD** (`hud/hud_tod.py`, prototype `hud/tod.html`, preview: HUD, **Panel by hour**):

- **The matrices.** The HD panel is the palace's. The game's sprites in each variant (`recolour/VARIANT/`) give the 15 ramp colours of the palace and their colours in the variant, one for one. A 3 x 4 matrix per variant (`out = M · [r, g, b, 1]`, 0-255) is fitted to those 15 pairs by least squares; mean error 1.8 to 5.2 levels out of 255. The matrices are in `hud/tod.json` (palace: identity), with the slots' variants and the pairs.
- **Applying it:** the matrix tints the whole panel layer (the panel rows, the collar and the head). Then what keeps its colours is drawn untinted on top. The head is drawn from its layers (`head.png`, then `collar-front.png`, in its region (136,124)-(184,176), section 4), not from the sprites `ICONES-016` to `026`: those are clipped at the engine's 20 x 23 rectangle at (150,137), which cuts the top of the HD head.
- **Between slots** the prototype blends the two matrices over the slot before a change. The game switches at the slot's start; **Game steps** does the same.
- **A ramp along the brightness** was tried too. It kept the HD's warm detail as a difference and oversaturated the slate; the matrix matched the game's mood better (`hud/tod/compare.png`).
- **In the game:** read which palette the place loaded: the room's sheet, or the sky slot (`[PALETTE_SUB]` after 388Dh). Apply the matching matrix to the panel's HD layers (a colour matrix in the compositor's shader), not to the room, and not to the fixed sprites.
- **Not covered:**
  - the day's palette group (8 groups of 4 sky palettes; the matrices are fitted to group 0);
  - the map screen's blue (a third kind of variant);
  - the fades, which the game makes by moving the palette.

## 10. How text is written

The game writes all its text (the commands, the day, the dialogue, the book, captions) glyph by glyph from one bitmap font. The HD layer must write the same strings, in the same boxes, in an HD typeface. It must not upscale the glyphs' pixels; the mock-ups' Garamond was a stand-in only.

**The game's font** (`DNCDPRG.lst`; the decompile's `video.rs`, `gfx.rs`, `tails.rs`):

- **The resource:** BBh (`DNCHAR.BIN`), or C7h (`DNCHAR2.BIN`) for the Fremen language 6. It is loaded at start-up and on a language change (`loc_0cfe4`) to DS:CEECh. It is 2304 bytes:

| Offset | What |
|---|---|
| 0 (CEECh) | the tall font's advance widths, a byte per code 0-127 |
| 80h (CF6Ch) | the small font's advance widths |
| 100h | the tall font's glyphs: 9 bytes each, a byte a row, the leftmost pixel the high bit (`[2514h]`) |
| 580h | the small font's glyphs: 7 bytes each (`[2516h]`) |

- **Two fonts:**
  - **tall** (9 rows, capitals 7 rows high): `font_select_tall_font`, D068h, glyph routine D096h;
  - **small** (7 rows, capitals 5 high: rows 1 to 5): `font_select_small_font`, D075h, glyph routine D12Fh.
  The selector sets the glyph routine `[2518h]` and the width table `[47A0h]`.
- **Drawing a glyph:** at the pen (`font_get_draw_position`); then the pen moves on by the glyph's advance (`[0D82Ch]`). The colour is the text colour word (`[TEXT_COLOUR]`, e.g. F1FAh for the day): ink and paper as palette indices.
- **Codes, not Unicode:** the game's text tables hold font codes. 32-90 are ASCII; 97-122 a-z; 64 is ¿; 91-96 and 123-127 are the accented letters (â, ê, î, ô, û, í, à, é, è, ù, ç: read from the glyphs, to confirm per language). A renderer maps the code to the glyph and never goes through Unicode.
- **Where the game writes:**
  - the commands: the small font, the pen at (97, 160 + 8k) for row k (checked on a frame: the capitals' ink at rows 161-165 in row 0, x from 97);
  - the day: the small font, `draw_number_at_pos`, then a space;
  - the dialogue and the book: the tall font (not measured yet).
  The live, dimmed and hovered commands differ only by their colour.

**The HD typeface** (`tripo/font/`; page `font/font.html`, preview: HUD, **HD font**):

- **A smooth typeface, not the game's blocks** (user, 2026-11-27): the HD text does not reproduce the pixel style. The page compares candidates on the panel. The default is **Open Sans Semibold**, under an open licence that allows shipping it with the game (Apache 2.0; `font/OpenSans-Semibold.ttf`). The alternatives:
  - **Lato Bold** and **Carlito Bold:** also open licences;
  - **Open Sans Bold:** heavier;
  - **Bahnschrift:** the shapes closest to the game's, but a Windows font that cannot be redistributed, shown for comparison only.
- **How close each is to the game:** all 589 fonts installed here were compared glyph by glyph against the game's tall font (`font_match.py`, `font/match.json`). Bahnschrift scores 0.749, Open Sans Semibold 0.740 and Lato 0.726, and no typeface has the game's squared shapes. The score only ranks closeness; the choice is a smooth, legible sans that holds up at small sizes.
- **The Fremen script** (`DNCHAR2.BIN`) has no typeface. It keeps the game's glyphs converted to strokes (`font_build.py` → `font/dune-hd-font.json`): each ink pixel joined to its side, upper or lower neighbours by a straight stroke, and to its diagonal ones when no shared neighbour is ink; drawn as round-capped lines of width `weight` (1.0 = the game's stroke). The same conversion of the Latin glyphs is on the page as "the game's glyphs as strokes", for comparison.

**Writing a string:**
1. **Get the whole string.** Take the string, its pen position, its font (tall or small, Latin or Fremen) and its colour word at the engine's text routine. Hook the routines that draw strings, such as `font_draw_phrase_or_command_string`, rather than the glyph routine, so the whole string is known.
2. **Map codes to letters.** ASCII is direct. The extra codes are 64 ¿, 91 â, 92 ê, 93 î, 94 ô, 95 û, 96 í, 123 à, 124 é, 125 è, 126 ù, 127 ç, read from the glyphs and to be confirmed per language. The table is `CODE_TO_CHAR` in `font/font.html`.
3. **Size and place.** The capitals are as high as the game's: 5 game pixels for the small font, 7 for the tall one. The font size is `cap height / the typeface's cap-height ratio`. The text stands on the game's baseline: row 6 of the small font's 7, row 7 of the tall font's 9, from the pen's y. It starts at the pen's x.
4. **Never wider than the game.** The string's width in the game is the sum of its codes' advances (the width tables). When the typeface's text is wider, squeeze it horizontally to that width; otherwise leave it at its natural width. Boxes and hit areas stay the game's.
5. **Colour:** map the game's ink index to the HD colour of that state, as on the panel:
   - live (236, 214, 170), dimmed (150, 124, 96);
   - a dark copy (20, 12, 8) offset 0.3 game pixels down and right, for the engraved look;
   - then the place's and the hour's colour matrix (section 9), as for the rest of the panel.
6. **Rendering:** render the typeface with the engine's text renderer (a multi-channel signed-distance-field atlas made once from the TTF serves every size), or rasterised per string at the output size.
7. **The day** is centred in the HD alcove rather than at the game's right-aligned x (section 6), in the small font's size.

**Not covered:**
- the measured places of the dialogue's and the book's text (tall font);
- the accented codes' letters per language, to confirm;
- the decorative gold title font (`GENERIC`, 64 large serif glyphs, 14 to 18 pixels, used for titles: a separate sheet, not yet done);
- an OTF/TTF export of the Fremen strokes, for tools that need a font file (needs `fontTools`, not installed here).

## 11. Drawing it in the game

- **Marks,** as the rooms and heads have:
  - the frieze: `redraw_frieze_screen` (B23Fh); the side sets (D795h and its entries D75Ah, D792h, D7ADh, D7B2h); the nudge (D23Dh); the button sets (D72Bh, D717h);
  - the slots: `draw_gauges` (D763h) and `blink_gauges` (D7B7h);
  - the head: 17BEh, 17E6h, 181Eh, 1843h;
  - the date: 1A34h;
  - the page turn: B1AFh;
  - the command rows (D42Fh to D443h and their text).
  Nothing marks a draw as HUD today: the HD layer knows a sprite only by the hash of its pixels, and most of the 74 `ICONES` pieces mean nothing alone.
- **Draw, don't upscale:** at a mark, draw the HD base (`hud/base.png`, or its pieces by the engine's rectangles) for the panel, then the overlays that the state turns on:
  - the left block's picture (`ICONES-000`, `006`, `009`);
  - the live arrows and the button;
  - the head's layers at its step;
  - the faces in the slots (or the empty slot when blinking);
  - the sun, the moon and the day;
  - the command text in the HD font.
  Skip the game's pixels for the panel rows.
- **What stands over the room** (the head and its collar, the open book's top, the turning page) has its own alpha and is drawn after the room.
- **The page turn** replaces the two sprites: on B1AFh, read DX and play the HD turn over the open book for the game's duration; the game then redraws the open book (element 1AF4h), which is the HD book again.
- **Colours:** the place's and the hour's colour matrix (section 9) on the panel's layers; the fades as a grade; the map screen's blue as a tint or a second master.

## 12. Assets

All game-derived, in `tripo/hud/` (outside this repo; the pictures are in no repo):

| Asset | File | Made by |
|---|---|---|
| the HD master (Banana Pro) and its states | `hud/master.png`, `hud/state-*.png`, `hud/overlay-*.png` | `hud-recipes.json`, `hud_input.py`, `hud_state.py` |
| the base | `hud/base.png` (5224 x 3264) | `hud_cut.py` |
| the pieces by the engine's rectangles | `hud/pieces/ICONES-000, 003, 006, 009, 014, 027, 033, 064` | `hud_cut.py` |
| the overlays | `hud/pieces/ICONES-029 to 032` (arrows open), `036` (palace plan) | `hud_state.py`, `hud_cut.py` |
| the head | `hud/pieces/ICONES-015`, `head.png`, `collar-front.png`, and `ICONES-016 to 026` | `hud_head.py`, `hud_cut.py` |
| the faces | `hud/pieces/ICONES-065 to 073` | `hud_faces.py` |
| the hawks | `hud/pieces/ICONES-053`, `076` | `hud_insignia.py` |
| the sun and the moon | `hud/pieces/ICONES-074`, `075`, `hud/sky.json`, `hud/sky-mask.png`, `hud/sky-bg.png` | `hud_sky.py` |
| the book's pages and crease | `hud/book-page-left.png`, `book-page-right.png`, `hud/book.json`, `hud/book-crease.png` (a check) | `hud_book.py` |
| the mock-ups | `hud/mock-room.png`, `hud/mock-book.png` | `hud_cut.py` |
| the HD typeface | `font/OpenSans-Semibold.ttf` (the default; Lato, Carlito, Open Sans Bold to compare), `font/dune-hd-font.json` (the Fremen strokes), `font/glyphs.json` (the game's pixels), `font/match.json` (the installed fonts' scores), `font/hd-sheet.png` | `font_ref.py` (from `DUNE.DAT`), `font_match.py`, `font_build.py` |
| the colours by place and hour | `hud/tod.json` (the matrices), `hud/tod/game-*.png`, `hud/tod/compare.png` | `hud_tod.py` (needs `recolour/` from `recolour_views.py`) |
| the palace plan | `hud/palplan/tablet.png`, `base.png`, `wall-3/4/5.png`, `mark.png`, `player.png`, `pieces.json`, `layout.json`, `frame-game.png` | `palplan_ref.py` (from a frame of the game), `palplan_gen.py`, `palplan_cut.py` |
| the Mixer Panel | see [hud-mixer-implementation.md](hud-mixer-implementation.md) | `mixer_ref.py`, `mixer_gen.py`, `mixer_cut.py` |

Order to rebuild: `hud_cut.py` (after any state), `hud_faces.py`, `hud_insignia.py`, `hud_sky.py`, `hud_book.py`. The prototypes: `hud/sky.html` (preview: HUD, **Day and night**), `hud/book.html` (**Page turn**), `hud/mixer.html` (**Mixer panel**), `hud/palplan.html` (**Palace plan**), `hud/tod.html` (**Panel by hour**), `font/font.html` (**HD font**), and the process view (**Process: the HD panel**).

## 13. Open points

- **Not done in HD:**
  - the map and travel button sets (1CCAh and 1D72h sprites 37-44, 49);
  - the nudge's own transition;
  - the map screen's blue panel;
  - the edge strips (12) and the in-between side sprites (not needed with layers).
- **To check in the running game:**
  - that `wait_a_bit` counts the engine's 200-a-second ticks (the page turn's length);
  - the page turn's direction against the decompile's "next page" (AFC7h on the left page);
  - the music buttons and other states not listed here.
- **The sun and moon's positions** in HD are not the game's (they follow the HD arch); the slots are.
