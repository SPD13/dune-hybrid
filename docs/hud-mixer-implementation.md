# The Mixer Panel and the language selector: how to implement them in the game

This is the specification for drawing the settings panel (the command **Mixer Panel**: the volumes, their dials, the language and the subtitle mode) in HD in the game. It is written from the prototype that does it today (`Cryogenic-Web/portrait-proto/studio/tripo/hud/mixer.html`, in the private repo `SPD13/dune-talking-heads`), and from the engine's own code: the decompile worktree's `hand/panels.rs`, `hand/ui.rs` and `hand/tails.rs`, `DNCDPRG.lst` (Cryogenic's `doc/`), and the tables in `DNCDPRG.EXE`. How the HD panel was made, and the rest of the HUD, are in [talking-heads-3d.md](talking-heads-3d.md) (the HUD pilot's table).

**Principle.** The game's data and logic stay as they are: the records, the clicks, the drags, the drivers' volumes, the language. Only the drawing changes. Every coordinate below is in the game's own pixels, relative to the panel's origin unless said otherwise, so the HD drawing can sit on top of the game's state and follow it.

**Status.** Read from the code and composed from the game's sprites; not yet checked against a frame of the running game (the composition, `hud/mixer_ref.py`, places every sprite by the engine's tables). Open points are listed in section 8.

## 1. What the game does

- **Opening.** The command phrase 9Eh ("Mixer Panel") is in the command table at DS:21F6h (pairs of phrase, routine) with the routine `loc_0a3f0`. It sets the screen's mouse handler list `[2570h] = 1AD6h`, calls `loc_0d2bd`, then falls into `settings_ui_draw` (A3F9h).
- **The handler list 1AD6h** (pairs of a word F66h and a routine): `settings_click` (A576h), `loc_0a5aa` (A5AAh: `[28BEh] = 0`, what the cursor holds cleared) and `settings_drag` (A5DFh), in that order; which event calls the second has not been traced (presumably the button's release).
- **Drawing** (`settings_ui_draw`, A3F9h), on the screen buffer (`set_screen_as_active_framebuffer`), resource 55h (`MIXR`):
  1. the panel, sprite 0, at the origin `[2886h], [2888h]` = (40, 1);
  2. the sliders (`settings_ui_draw_volume_sliders`, A4C6h);
  3. the dials (`loc_0a47d`);
  4. the lit language (`settings_ui_draw_language_buttons`, A42Ch);
  5. the lit mode (`loc_0a44c`);
  6. the music buttons (`loc_0ac3a`: the three buttons of the list 201Ah hidden, bit 40h; shown again with the music on; what they are is not settled);
  7. then `loc_0d32f` with BX = A541h: a redraw asked, the handlers above the level unwound, the panel faded in.
- **Closing.** A click outside the panel's rectangle `[2886h]` (40,1)-(250,144) leaves it (`leave_and_fade`, D2E2h). The close routine `sub_a541` (A541h) saves the mode (`[28E8h] = [28E7h]`), clears a mouse rectangle, puts the map's click handler back (`set_map_click_handler`) and redraws the rectangle 2886h from what is under it (`draw_head_if_needed_and_update_screen_rect_at_si`): the room comes back.
- **Every change redraws only its part:** a slider (A502h), a dial (A49Ch), or the whole panel after a language or mode click (A3F9h again).

## 2. The data

All in the data segment (DS:0 is `DNCDPRG.EXE` file offset F6B0h).

| Address | What | Values at the start |
|---|---|---|
| `[2886h]` | the panel's rectangle: x0, y0, x1, y1 (screen) | (40, 1, 250, 144) |
| 288Eh, 2896h, 289Eh | the three **slider** records, 8 bytes each, left to right | values 255, 230, 180 |
| 28A6h, 28AEh, 28B6h | the three **dial** records, 8 bytes each, under the sliders | values 100, 120, 140 |
| `[28BEh]` | what the cursor holds: 0 nothing, 1 a slider, 2 a dial | 0 |
| `[28BFh]` | the voice test box: x0, y0, x1, y1 (panel) | (31, 122, 90, 132) |
| `[28C7h]` | the list's rectangle: x0, y0, x1, y1 (panel) | (134, 39, 200, 130) |
| 28CFh | the list's 13 rows: the entry of each row | 0, 3, 1, 2, 4, 5, 6, 7, 7, 7, 8, 9, 10 |
| 28DCh | each entry's row (the reverse of 28CFh) | 0, 2, 3, 1, 4, 5, 6, 7, 10, 11, 12 |
| `[0CEEBh]` | the language, 0 to 6 | the game's choice |
| `[28E7h]` | the subtitle mode, 0 to 2 (entry 8 + mode) | 2 for languages 0 and 3 with PCM voice (`check_amr_or_eng_language`, CFA0h) |
| `[28E8h]` | the mode kept while the map or a dialogue forces another; saved on closing | |
| `[0DBC8h]` | the sound settings bits (which parts are drawn, below) | |

**A slider or dial record** (8 bytes): +0 the value (a byte); +1 shown (1 once drawn, 0 when its channel is off: it then ignores the cursor); +2 x (word, panel); +4 y (word, panel; for a slider, the drawing writes its handle's y there); +6 the routine called after a change: A637h for the left column, A650h for the middle one, D917h for the right one (a bare `ret`: the right column's values are only read when a voice is played, by ADE0h). A slider and the dial under it share the routine: the dial is the second value of the same channel.

| Column | Slider (x, track at y 22h) | Dial (x, y) | Routine | The art's heading |
|---|---|---|---|---|
| left | 288Eh at x 12 | 28A6h at (17, 103) | A637h: the far function at 39A5h with AL = the slider, AH = the dial; the slider forced to FFh when settings bit 4 is off | VOICES |
| middle | 2896h at x 50 | 28AEh at (55, 103) | A650h: the far function at 3985h with AL = the slider (at least 4), AH = the dial; the middle and right sliders forced to FFh when bit 400h is off | MUSIC |
| right | 289Eh at x 88 | 28B6h at (93, 103) | D917h, a bare `ret`; the values are used by ADE0h when a voice is played (the far function at 397Dh with AX = 64h, BL = the slider, at least 4, BH = the dial) | MUSIC during voices |

The decompile names A637h `settings_apply_music_volume` and the records "voice, music, music 2"; the art's headings (VOICES at the left) and these names do not agree, and which driver each far function is has not been checked. The implementation does not need it: it draws the values, the game applies them.

**What is drawn, by the settings bits `[0DBC8h]`:**
- the left slider when PCM is enabled; marked not shown unless bit 4;
- the middle and right sliders when the music is on (`loc_0ae28`); both marked not shown unless bit 400h;
- the left dial with bit 8; the middle and right dials with bit 800h.
A part that is not drawn leaves the panel's own art in its place (the empty groove, the dial's stand).

## 3. The elements and their geometry

Panel pixels; add (40, 1) for the screen.

| Element | Sprite (`MIXR`) | Rectangle | Drawn |
|---|---|---|---|
| the panel | 0 (210 x 142) | (0, 0)-(210, 142) | at the origin |
| a slider's track | 1 (21 x 69) | (x, 34)-(x + 21, 103) | before its handle, erasing the old one |
| a slider's handle | 2 (22 x 5) | (x, hy)-(x + 22, hy + 5), `hy = 22h + ((~value) & FFh) >> 2` | over its track; hy goes from 34 (value 255, the top) to 97 (value 0..3) |
| a dial | 3 + value / 10 (13 x 11), 25 pictures | (x, y)-(x + 13, y + 11) | values 0 to 240; the picture turns from lower left (3) through straight up (15) to lower right (27), 11.25 degrees a step |
| a list entry, lit | 1Ch + 2n (65 x 7) | (134, 39 + 7 r)-(199, 46 + 7 r), r = row of n (28DCh) | entry n: the language, or 8 + the mode |
| a list entry, unlit | 1Dh + 2n (65 x 7) | same | part of the panel's art; never drawn by the code |
| the voice test | (in the panel) | hit box (31, 122)-(90, 132) | the plate is drawn about 11 pixels right of the hit box |

**The list**, 13 rows of 7 pixels from y 39:

| Row | Entry | Lit sprite | Text | Resources it loads |
|---|---|---|---|---|
| 0 | language 0 | 1Ch (28) | AMERICAN | |
| 1 | language 3 | 22h (34) | ENGLISH | |
| 2 | language 1 | 1Eh (30) | FRANCAIS | |
| 3 | language 2 | 20h (32) | DEUTSCH | |
| 4 | language 4 | 24h (36) | ITALIANO | |
| 5 | language 5 | 26h (38) | ESPAÑOL | |
| 6 | language 6 | 28h (40) | FREMEN | the font C7h in place of BBh |
| 7, 8, 9 | 7: nothing | (2Ah: blank) | | |
| 10 | mode 0 | 2Ch (44) | Subtitles | |
| 11 | mode 1 | 2Eh (46) | Balloons | |
| 12 | mode 2 | 30h (48) | No Subtitles | |

Sprites 50 to 53 (CHINESE and JAPANESE, lit and unlit) are in the sheet but in no row.

**The arrows of the lit entries** (in the lit sprites; the HD arrows go at the same columns, 0 the sprite's left = panel x 134): ▶ at columns 9-12 and ◀ at 54-57 for the seven languages and Subtitles; Balloons 7-10 and 55-58; No Subtitles 3-6 and 60-63.

## 4. The input rules

- **A click** (`settings_click`, A576h; the cursor made panel-relative by `settings_ui_sub_global_offset`):
  - outside the panel: leave (section 1);
  - in the voice test box: with PCM, the test voice (`create_voc_file_name_from_bx` AX 4, BX 5; `voc_get_lipsync_data`; the voice volume applied; `[VOC_PCM_PLAYING] = 1`; played through the sound driver; waits for its end);
  - in the list: row = (y - 39) / 7, entry = 28CFh[row]. 7: nothing. Above 7: `[28E7h] = entry - 8`, the panel redrawn. Below 7 and not the current language: bit 1 of `[28E7h]` cleared, `[0CEEBh] = entry`, the language's resources loaded (`loc_0cfe4`: the font resource BBh, or C7h for language 6, into CEECh; the commands C0h + language into `[47ACh]`; the phrases 9Ah + language), the panel redrawn;
  - else `[28BEh]` = 1 when on a slider's handle (16h x 5 from its x and handle y, if shown), 2 when on a dial (0Dh x 0Bh, if shown), 0.
- **A drag** (`settings_drag`, A5DFh; CX, DI the mouse's moves):
  - holding a slider: `a = handle_y + dy - 22h`; if `a < 40h`, `value = ~(a << 2)` (that is 255 - 4a), the slider redrawn, its routine called (+6);
  - holding a dial: the move's sign is turned so that going round the dial clockwise counts up: the x move flipped when the cursor is in the dial's left 6 pixels, the y move flipped when in its bottom part (from 5 down); their sum's sign gives +10 or -10; the value is kept when the result would be F1h or more (so 0 to 240); the dial redrawn, its routine called;
  - holding nothing: nothing.
- **A5AAh** (presumably on the release) clears `[28BEh]`.

## 5. The HD panel

Made by Banana Pro through Google (one master from the game's panel, in the style of our HD HUD; one edit without the handles and dials, which also put the tilde on ESPAÑOL back), cut by `hud/mixer_cut.py`. Scale: K = 5224 / 320 = 16.325 pixels to one game pixel (the HUD master's scale). All placements below are in game pixels of the panel, as the prototype draws them.

| Layer | File | Size (pixels) | Where (panel, game px) |
|---|---|---|---|
| the base: the panel with empty grooves and no dials | `hud/mixer/base.png` | 3428 x 2351 | at (0, 0), 210 x 144 |
| a handle | `hud/mixer/handle.png` | 276 x 110 | at (x + 2.46, hy - 2.58), 16.91 x 6.74; hy as in section 3 |
| a dial | `hud/mixer/knob.png` (pointer straight up) | 181 x 181 | centred on (x + 5.92, y + 5.58), 11.09 across, turned by `-135 + 11.25 * (value div 10)` degrees (clockwise) |
| a dial's shadow | drawn in code | | a disc just inside the dial, colour (20, 8, 6) at 75 %, blurred 0.9 px, offset (+0.25, +0.55); fixed, it does not turn |
| a lit entry | `hud/mixer/lit-NN.png`, NN the entry (00-06, 08-10) | 1126 x 114 | at (132, 39 + 7 r + shift_r), 69 x 7; the shifts per row (the HD words are not exactly on the game's rows): rows 0-6: 0.2, 0.2, 0.1, 0.3, 0.9, 0.3, 0.6; rows 10-12: 0.2, 0.7, 0.7 |

**The lit entries** are drawn in code from the base, not generated (the model's lit edit lit the empty row and turned the arrows the wrong way): the base's engraved letters, their strokes filled, turned cream with an orange glow; a faint band behind the word, the same on every row of a block (languages x 145.1 to 191.0; modes x 138.3 to 192.8, as wide as No Subtitles, each band clipped to its row's slate); the arrows at the game's columns (section 3). Nothing is drawn outside the row's slate.

**What is not in the HD panel:** CHINESE and JAPANESE (in no row), the music buttons of `loc_0ac3a`, the voice test's own feedback (the prototype only lights the plate a moment; the game plays a voice and shows nothing).

## 6. Drawing it in the game

- **Mark the routines,** as the rooms and heads have marks: `settings_ui_draw` (A3F9h), `settings_ui_draw_slider` (A502h), `draw_counter` (A49Ch), the lit buttons (A42Ch and A44Ch), and the close `sub_a541` (A541h). At each mark, read the records of section 2; nothing needs to be decoded from the pixels.
- **Draw, don't upscale.** On A3F9h, draw the base at the panel's origin times the output scale; then each shown slider's handle and each shown dial (section 5); then the lit language and the lit mode. Skip the `MIXR` sprites' pixels for that rectangle. On A502h or A49Ch, redraw that column only (the base under it, then its handle or dial). On A541h, drop the panel's layer: the room and the HUD under it are drawn again by the game.
- **Not shown means the base:** a slider or dial whose record has +1 = 0 (its channel off) is not drawn; the base already shows the empty groove or the dial's stand.
- **The fade** (`loc_0d32f`) and the screen's palette changes apply to the HD layer as to the rest of the HUD (see talking-heads-3d.md: the panel's colours are not fixed in the game).
- **The input stays the game's:** the hit boxes, the drags and the values are the engine's; the HD drawing only follows `[28BEh]`, the records, `[0CEEBh]` and `[28E7h]`. A smoother drawing is possible because the HD handle and dial can sit between the game's steps (the dial turns by any angle), but the values must stay the game's (handle steps of 4, dial steps of 10).

## 7. Assets and how to rebuild them

All game-derived (made from the game's panel), in `tripo/hud/` (outside this repo; the pictures are in no repo):

| Step | Script | Output |
|---|---|---|
| the game's panel, as the engine composes it, and its layout | `hud/mixer_ref.py` (reads `DNCDPRG.EXE` and the `MIXR` sprites) | `hud/mixer/ref-game.png`, `ref-annotated.png`, `ref-panel-plain.png`, `layout.json` |
| the HD master and the bare edit | `hud/mixer_gen.py master`, `hud/mixer_gen.py bare` (recipe `hud/mixer-recipes.json`; Google, about $0.24 each) | `hud/mixer/master.png`, `state-bare.png` (5056 x 3392) |
| the pieces | `hud/mixer_cut.py` | `base.png`, `handle.png`, `knob.png`, `lit-NN.png`, `unlit-NN.png`, `hd-panel.png`, `pieces.json` (every number of section 5) |
| the prototype | `hud/mixer.html` (preview: HUD, **Mixer panel**) | |

Order: `mixer_ref.py`, then `mixer_gen.py master` and `bare` (only to remake the art), then `mixer_cut.py`.

## 8. Open points

- **Not checked against the running game:** the composition follows the code and the tables; a frame of the real panel (`dune-run` with the Mixer Panel command) would confirm the sprites' places, the knob angles and the lit entries.
- **What each column drives:** A637h and A650h call the far functions 39A5h and 3985h at once; the right column's D917h does nothing, its values being read by ADE0h (far 397Dh) when a voice plays; the decompile's names (music, sound, voice) and the art's headings (VOICES, MUSIC, MUSIC during voices) disagree. Not needed for the drawing.
- **The subtitle modes:** 0, 1, 2 are the rows Subtitles, Balloons, No Subtitles; the decompile notes that with 0 no text box is drawn (8C98h), which does not match the label "Subtitles". To check in the running game.
- **The music buttons** of `loc_0ac3a` (list 201Ah): what they are and where they are drawn.
- **The handle's exact pixel** at values that are not multiples of 4: the game rounds `(~value) >> 2`; the HD handle follows the same rule.
