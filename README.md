# Dune Hybrid

**Cryo's *Dune* (1992, CD version) in your web browser.** The page runs the original DOS program on a small PC emulator written in Rust and compiled to WebAssembly. It works on desktop, tablets and phones (touch controls, save anywhere, installable as an app) and runs offline once loaded.

> **The game itself is not included.** You need the original game files (`DNCDPRG.EXE` and `DUNE.DAT` from the Dune CD). See [Installation](#installation).

- [What's new](#whats-new)
- [Features](#features)
- [Installation](#installation)
- [Remastered soundtrack (optional)](#remastered-soundtrack-optional)
- [Playing](#playing)
- [How it works](#how-it-works)
- [Building and running it yourself](#building-and-running-it-yourself)
- [Development](#development)
- [Roadmap](#roadmap)
- [Credits and licenses](#credits-and-licenses)

---

## What's new

Things this edition adds that were not part of the original 1992 game:

- **Play on any device with a web browser.** The original needed an MS-DOS PC with a CD-ROM drive, a VGA card and a Sound Blaster. It now runs on Windows, macOS, Linux and ChromeOS computers, Android phones and tablets, iPhones and iPads, with nothing to install and no DOS or emulator setup. It is built on standard web technology meant for any recent browser (Chrome, Edge, Safari, Firefox). So far it has been tested in Chrome on desktop; reports from other browsers and real phones are welcome.
- **Mobile support.** The game is fully playable on phones and tablets:
  - touch controls, with a choice of direct tapping or trackpad-style control;
  - on-screen buttons for the keys the game needs;
  - landscape and portrait layouts that respect notches and rounded corners;
  - fullscreen, and the screen kept awake while you play;
  - installation to the home screen;
  - automatic saving of your exact position whenever you switch apps.

  See [Mobile controls and gestures](#mobile-controls-and-gestures).
- **Picture filters.** The 320×200 picture can be redrawn by pixel-art upscalers (MMPX, Scale4x, xBR), drawn at your screen's full resolution with sharp or smooth scaling, and given scanlines or a full CRT monitor look. Pick a preset or combine the filters yourself; changes apply instantly while you play. See [Picture filters](#picture-filters).
- **Remastered soundtrack.** In 2024 the game's composer, Stéphane Picq, released a remaster of the original music, including re-rendered versions of the game's own songs. If you buy it, the game can play those recordings instead of the 1992 AdLib/FM music. They stay in sync with the action, dip under voices and fade exactly as the game's music does. See [Remastered soundtrack](#remastered-soundtrack-optional).

## Features

- **The complete original game**: intro, full speech, AdLib/OPL3 music, Sound Blaster voices and effects. It runs at the original speed, with the original game code, so nothing is missing or reinterpreted.
- **Runs in any modern browser**: no plugins and no installation; the app is about 120 KB gzipped plus your game files.
- **Your files stay on your device**: game files and saves are kept in the browser's private storage and never uploaded.
- **Import straight from the CD image**: pick the `.iso`, a `.zip` containing it, or the two game files.
- **Touch controls** with two modes:
  - *direct*: tap where you want to click;
  - *trackpad*: drag the cursor, tap to click, two-finger tap to right-click.
- **On-screen buttons**: Esc / Enter / Space / Pause keys, fast-forward, quick save and load, and fullscreen.
- **Save anywhere**: three snapshot slots with thumbnails, alongside the game's own saves.
- **Continue where you left off**: the game state is saved every 30 seconds and whenever you switch away. Phones that discard background tabs lose nothing.
- **Six languages** for text (English, French, German, Italian, Spanish, Dutch), using the game's built-in translations.
- **Separate music and voice volume**.
- **Picture filters**, switchable live: pixel-art upscalers (MMPX, Scale4x, xBR), sharp or smooth scaling at full screen resolution, scanlines or a CRT monitor effect.
- **Battery saver**: skips the time the original program spends busy-waiting. This uses about 4× less CPU, and the picture stays pixel-for-pixel the same.
- **Installable app (PWA)** that works offline after the first visit.
- **Optional remastered soundtrack** (sold separately by the composer) in place of the FM music.

## Installation

### 1. Get the game files

This project ships **no game data**. *Dune* is © Cryo Interactive Entertainment and its successors; the project is not affiliated with or endorsed by any rights holder.

You need the **CD-ROM version** of Dune (the version with the movie clips and full speech). This project targets release 3.7; other CD releases may work. The floppy version will **not** work, because it has no `DUNE.DAT`.

Options:

- **Your own CD.** Copy `DNCDPRG.EXE` and `DUNE.DAT` from it, or make an ISO image of the disc.
- **Abandonware.** The game is no longer sold. MyAbandonware hosts it at <https://www.myabandonware.com/game/dune-23h>:
  - download **"ISO Version (English)"** (about 214 MB), which is the CD image;
  - the small "RIP" versions are the floppy game and won't work.

  "Abandonware" is a description, not a license. Check the rules in your country before downloading, and prefer an original copy if you can.

### 2. Open the app

Either open a hosted copy of this project, or run it yourself (see [Building and running it yourself](#building-and-running-it-yourself)).

### 3. Import the files (once)

On the title screen, choose **Import game files…** and select one of these:

| What you have | Select |
|---|---|
| The CD image | the `.iso` file |
| A download containing the CD image | the `.zip` file (stored or deflate compression) |
| The files themselves | both `DNCDPRG.EXE` and `DUNE.DAT` |

The files are copied into the browser's private storage (about 400 MB) and then checked:
- "Game files ready: Dune CD version 3.7" means you have the exact version this project is tested with.
- Another `DNCDPRG.EXE` produces a warning but may still work.

**Not supported:** 7-Zip/RAR archives and BIN/CUE images; extract or convert them first. If the browser can't store files (some private modes), the game runs from the selected files for that session only.

Afterwards, **New game** starts from the intro, and **Continue** resumes your last session.

## Remastered soundtrack (optional)

The game's composer, **Stéphane Picq**, released **[Dune Spice Opera 2024 remaster](https://stphanepicq.bandcamp.com/album/dune-spice-opera-2024-remaster-lp)** (EXXOS, Stéphane Picq with Philippe Ulrich). Besides the remastered *Spice Opera* album, it contains the **full in-game soundtrack**: the game's own songs re-rendered in high quality, labelled with the game's internal names (`PC_ARRAKIS`, `PC_MORNING`, …). This app can play those recordings instead of the original AdLib/OPL3 FM music.

> ⚠️ **This music is copyrighted and is not included in this project.** To use it you must **buy your own copy** from the composer's Bandcamp page:
> **<https://stphanepicq.bandcamp.com/album/dune-spice-opera-2024-remaster-lp>**
> Please support the artist; do not share the files.

### How to get and use it

1. **Buy** the album on [Bandcamp](https://stphanepicq.bandcamp.com/album/dune-spice-opera-2024-remaster-lp).
2. **Download** it from Bandcamp in the **MP3** format. You get a ZIP file named like `EXXOS (Stéphane Picq with Philippe Ulrich) - Dune Spice Opera 2024 remaster_MP3.zip`, about 240 MB.
3. **Import it** in the app: on the title screen, in the **Remastered soundtrack** card, choose **Import soundtrack ZIP…** and select that ZIP file. Don't extract it.
4. The card lists the game songs that now use the remastered recordings. The option **Remastered soundtrack** (title screen and in-game menu) switches between the remaster and the original FM music at any time.

### Track mapping

The album has 22 tracks. The app uses only the **eight game tracks (13–20)**. It recognises them by the `PC_<NAME>` label in their file name and MP3 title tag, which is the game's internal name for each song:

| Album track | Length | Replaces in-game song | Kept on the device |
|---|---|---|---|
| 13 · PC_ARRAKIS [2024 remaster] | 3:09 | ARRAKIS (palace) | ✅ |
| 14 · PC_BAGDAD [2024 remaster] | 4:18 | BAGDAD | ✅ |
| 15 · PC_MORNING [2024 remaster] | 4:57 | MORNING (intro) | ✅ |
| 16 · PC_SEKENCE [2024 remaster] | 3:33 | SEKENCE | ✅ |
| 17 · PC_SIETCH [2024 remaster] | 1:30 | SIETCHM | ✅ |
| 18 · PC_WARSONG [2024 remaster] | 1:21 | WARSONG | ✅ |
| 19 · PC_WATER [2024 remaster] | 1:24 | WATER | ✅ |
| 20 · PC_WORMINTR+WORMSUIT [2024 remaster] | 6:24 | WORMINTR (0:00–2:52.4) and WORMSUIT (2:52.4–end) | ✅ |
| — | — | CRYOMUS (Cryo logo jingle): no remaster, the original FM music plays | — |
| 1–12 · *Spice Opera* album (Dune Theme, Emotion Control, Ecolove, Water Of Life, Revelation, Free Men, Wake Up, Too!, Chani's Eyes, Sign Of The Worm, Spice Opéra, Dune Variation) | — | not used | ❌ |
| 21–22 · Bonus tracks (Dune Theme overclocked, Cryogenia) | — | not used | ❌ |
| Cover images (3 JPEG files) | — | not used | ❌ |

**Only the eight tracks marked ✅ are kept on your device** (about 62 MB), copied into the browser's private storage.
- **Not stored:** the album arrangements, the bonus tracks and the cover images. The ZIP itself isn't kept either; the app reads the eight tracks out of it during the import and then no longer needs it.
- **Why the album arrangements aren't used:** they are longer studio versions that don't follow the game's timing.
- **Not uploaded:** nothing is sent anywhere.
- **Removing:** **Remove soundtrack** on the title screen deletes the eight stored tracks.

## Playing

### Controls

| Input | Action |
|---|---|
| Mouse | As in the original: point and click. |
| Keyboard | As in the original. Esc skips scenes, P pauses the game. **F1** opens the menu. |
| Touch: direct mode | Touch where you want to click. Dragging moves the cursor with your finger. |
| Touch: trackpad mode | Drag anywhere to move the cursor, tap to click at the cursor, two-finger tap to right-click. More precise on small screens. |
| On-screen buttons | **Left:** Esc, Enter, Space, P. **Right:** ☰ menu, ⏩ fast-forward (hold), ⤓ quick save to slot 1, ⤒ quick load slot 1, ⛶ fullscreen. |

The on-screen buttons appear automatically on touch devices; change this under **Options**. On a wide screen they sit beside the picture. In portrait they sit below it.

### Mobile controls and gestures

The game is played with a mouse pointer, so on a touchscreen your finger drives that pointer. Choose how under **Options → Touch control**.

**Direct mode** (the default): the pointer is where your finger is.

| Gesture | Effect |
|---|---|
| Tap | Moves the pointer there and clicks (left button). |
| Touch and drag | Holds the left button down while the pointer follows your finger, then releases when you lift it. |

The game sees the pointer move before the button goes down, so menus highlight correctly. A press always lasts at least 60 ms, so even very quick taps register.

**Trackpad mode**: the screen works like a laptop touchpad. It's more precise on small phones, where a finger hides the menu text it touches.

| Gesture | Effect |
|---|---|
| One-finger drag, anywhere | Moves the pointer by the drag distance, about 1.6× faster than your finger. |
| Quick tap (under ¼ s, almost no movement) | Clicks where the pointer is, not under your finger. |
| Two-finger tap | Right click. |

Drags never click, so you can reposition the pointer freely before tapping.

**On-screen buttons:**

| Button | Effect |
|---|---|
| **Esc** | Skip a cutscene or the intro, leave a screen (the original Esc key). |
| **↵** (Enter) and **␣** (Space) | The original Enter and Space keys. |
| **P** | The game's own pause. |
| **☰** | Opens the menu and pauses everything: save slots, options, fullscreen, back to title. |
| **⏩** (hold) | Fast-forward at 4× speed while held, for long flights and travel; sound is muted meanwhile. |
| **⤓** / **⤒** | Quick save to slot 1 / quick load from slot 1. |
| **⛶** | Fullscreen. On phones that support it, this also locks the screen to landscape. |

**Layout and behaviour on phones:**
- **Landscape** (recommended): the picture fills the screen height, with the buttons in the side margins.
- **Portrait:** the picture is at the top, with the buttons below within reach of your thumbs.
- Notches, rounded corners and home indicators are avoided automatically.
- **Page behaviour:** the game area does not scroll or zoom when touched, and double-tap zoom is disabled.
- **Sound** starts with your first tap; browsers require a touch before playing audio.
- **Background:** switching apps or locking the phone pauses the game and saves a **Continue** point. Coming back resumes exactly there, even if the system closed the tab meanwhile.
- **Screen:** it stays awake while you play.
- **For the best experience**, add the app to your home screen (see [Install as an app](#install-as-an-app)). It then starts fullscreen in landscape, without browser bars.

### Saving

There are two independent ways to save:

- **The game's own save system** (in the game's menus). Saves are stored in the browser and reloaded automatically.
- **Snapshots**: open the menu (☰ or F1) to **save anywhere** into three slots and load them back instantly. A snapshot captures the whole emulated PC, so it works mid-dialogue, mid-flight, anywhere. The **Continue** point is also a snapshot, taken automatically every 30 seconds and whenever the page is hidden.

Snapshots belong to the exact `DNCDPRG.EXE` they were taken with.

### Options

These are available from the title screen and from the in-game menu:
- **Language:** affects new games. A resumed snapshot keeps the language it was started with.
- **Music volume** and **voices & effects volume**.
- **Graphics:** a preset, or your own combination of filters. See [Picture filters](#picture-filters).
- **Touch control:** direct or trackpad.
- **On-screen buttons:** automatic on touch devices, always, or never.
- **Battery saver.**

### Picture filters

The game draws a 320×200 picture. The app shows it at the original 4:3 shape, rendered at your screen's full resolution with WebGL2, through up to three filters:

| Step | Choices |
|---|---|
| **Upscaler**: redraws the picture at a higher resolution before it is scaled | *None*; *MMPX*, a pixel-art upscaler that rounds diagonals and curves but only uses the picture's own colours; *Scale4x (EPX)*, more conservative; *xBR*, smooth anti-aliased edges with a painted look |
| **Scaling to the screen** | *Sharp*: crisp pixels without uneven sizes; *Nearest pixel*; *Smooth* |
| **Monitor** | *None*; *Scanlines*; *CRT*: Timothy Lottes' CRT shader with curvature, scanlines, shadow mask and glow (it works from the original picture, so it replaces the upscaler) |

Presets combine them:

| Preset | Upscaler | Scaling | Monitor |
|---|---|---|---|
| Original (default) | none | sharp | none |
| Smooth | none | smooth | none |
| Scanlines | none | sharp | scanlines |
| CRT monitor | none | sharp | CRT |
| Pixel-art HD | MMPX | sharp | none |
| Smooth HD | xBR | smooth | none |

Under **Options → Graphics → Filters** each step can be changed on its own (the preset then shows *Custom*), and a slider sets the strength of the monitor effect. The filters change only what you see: the game, its snapshots and their thumbnails are unaffected. Browsers without WebGL2 fall back to a basic display with CSS smoothing and scanlines.

### Install as an app

In Chrome/Edge use **Install app**; on iOS Safari use **Share → Add to Home Screen**. The app then opens fullscreen in landscape and works offline. The game files stay in the browser's storage for that site.

### Troubleshooting

- **No sound:** browsers start audio only after a click or tap. Click inside the game once.
- **Choppy on an old phone:** keep the battery saver on, and close other tabs.
- **"Could not restore":** the snapshot was taken with a different `DNCDPRG.EXE`.
- **Remove everything:** **Remove stored files** on the title screen deletes the game files. Clearing the site's data in the browser removes saves and snapshots too.

## How it works

```
 Browser page (TypeScript)                Web Worker (WebAssembly, Rust)
┌───────────────────────────┐   input   ┌───────────────────────────────────────────────┐
│ title screen, import      │ ────────► │  Emu (crates/web)                              │
│ canvas renderer           │           │   └─ Machine (crates/pc)                       │
│ touch / mouse / keyboard  │ ◄──────── │       ├─ CPU: 80286 interpreter (crates/cpu)   │
│ menu, snapshots, options  │   frames  │       ├─ memory 1 MB + HMA                     │
│ IndexedDB + OPFS storage  │   audio   │       ├─ PIC 8259 · PIT 8254 · keyboard · VGA  │
└──────────┬────────────────┘           │       ├─ OPL3 (oplon) · Sound Blaster Pro · DMA│
           │ samples                    │       └─ BIOS / DOS / mouse services in Rust   │
           ▼                            │  runs DNCDPRG.EXE, unmodified                  │
     AudioWorklet (48 kHz)              └───────────────────────────────────────────────┘
```

### The emulated PC

- **CPU** (`crates/cpu`): a real-mode 80286 interpreter, i.e. the 8086 instruction set plus the 186/286 additions.
  - It is validated against the **SingleStepTests 80286** suite: 1.44 million instructions recorded from a real chip, all matching.
  - Exceptions, documented in the tests: a few 286 protection faults and two silicon quirks that DOS programs never rely on.
  - It reports itself as a 286 to the game's CPU detection.
- **Hardware** (`crates/pc/src/hw.rs`): interrupt controllers, the programmable timer (the game reprograms it to 200 Hz), the keyboard controller, and VGA mode 13h with its palette and retrace status.
- **Sound** (`crates/pc/src/sound.rs`):
  - an **OPL3** FM chip at port 388h: ports, timers and status are emulated here, the synthesis comes from the MIT-licensed [`oplon`](https://codeberg.org/sbechet/oplon);
  - a **Sound Blaster Pro** at 220h, IRQ 7, DMA 1;
  - the **8237 DMA controller** that feeds it.

  The game's own AdLib and Sound Blaster drivers, loaded from `DUNE.DAT`, run unmodified on top. The game is started with `ADP330 SBP2227 <LANG>`, the same sound settings as the Cryogenic reference.
- **BIOS and DOS** (`bios.rs`, `dos.rs`): written in Rust and reached through small stubs in the emulated ROM. The program can still hook and chain interrupt vectors exactly as on real hardware. The services implemented are the ones the game uses:
  - the MZ loader, PSP and environment;
  - file access (read-only to DUNE.DAT, read-write to save files);
  - a Microsoft-compatible mouse driver;
  - the video BIOS;
  - the timer and keyboard interrupts.
- **The program is loaded at segment 1000h**, as in Spice86/Cryogenic, so addresses match the reference disassembly.

### Time, determinism and the battery saver

- **Virtual time:** time inside the emulator is derived from the instruction count, at 20 million instructions per second (a fast 486). The same inputs therefore always produce the same run, which makes testing and snapshots reliable.
- **Keeping pace with the clock:** the browser worker advances virtual time by the wall-clock time that has passed; sound is generated in step with virtual time.
- **Battery saver:** the original program waits for the timer by asking the mouse driver for its position in a tight loop, roughly 135,000 times a second. When the emulator sees about 32 such back-to-back polls with nothing changing, it jumps ahead to the next timer event, exactly as it does for a halted CPU. In the intro this skips about 74% of the time, and the output is identical pixel for pixel.

### Snapshots

`Machine::save_state` writes the whole PC into a compact, deflate-compressed blob of about 220 KB:
- CPU, memory and every device;
- open DOS files, by name and position;
- the OPL chip, rebuilt on restore from a shadow copy of its registers.

A save/restore round trip is bit-exact: a run interrupted by one ends in exactly the same state as an uninterrupted one (`crates/pc/tests/snapshot.rs`).

### Remastered soundtrack

- **Finding the tracks:** the importer finds the game tracks in the purchased ZIP by their `PC_<NAME>` labels. These appear in the file names and in the MP3 title tags (tracks 13–20).
- **Following the game's music calls:** the game drives its music driver through a table of far pointers at `DS:3971h` (play, with the song data at ES:SI), `DS:3975h` (stop) and `DS:397Dh` (fade to a volume over a number of 200 Hz ticks). The emulator watches those entry points (`crates/pc/src/music.rs`) and identifies each song by comparing its data with the songs in DUNE.DAT. The page plays the matching recording, in sync, and mutes only that song's FM rendition.
- **Volume:** the game lowers its music under voices (volume 230 → 140 over 0.5 s, back to 230 over 2 s) and fades it out between scenes. The recording follows the same ramps.
- **Song endings:** when the game's song ends by itself, the emulator notices its (still synthesized, muted) FM output going silent and the recording fades out. If the recording ends first, it loops.
- **Verified sync:**
  - The game's in-game recording of MORNING matches PC_MORNING at the same tempo (within 0.4%) from the same starting point.
  - The lengths computed from the game's song data (`tools/song-length.py`) match the remastered tracks, with each loop section played once.
  - The worm track is split where WORMINTR ends according to the song data (172.1 s), adjusted to the remaster's audible transition at 172.4 s.

### The web app

- **Game files** are stored in the **Origin Private File System**. The worker reads `DUNE.DAT` on demand through `FileReaderSync`, so the 400 MB file is never loaded into memory.
- **Imports** of `.iso` and `.zip` files are parsed in the page:
  - ISO 9660 directory walk;
  - ZIP central directory, with `DecompressionStream` for deflate.
- **Save games and snapshots** live in **IndexedDB**.
- **Frames** (palette indices plus the palette) and **audio** (48 kHz stereo, played by an AudioWorklet with ~85 ms of buffering) are posted from the worker to the page.
- **Rendering** (`web/src/render/`) is a WebGL2 chain. The indices go to an 8-bit integer texture and the palette to a 256×1 texture. Then:
  1. a palette pass produces RGB;
  2. an optional upscaler pass (or two for Scale4x) runs: MMPX at 2×, Scale2x twice, or xBR-lv2 at 4×;
  3. a final pass scales to the canvas at device resolution: nearest, sharp-bilinear or bilinear, with optional scanlines; with the CRT preset, crt-lottes runs here instead.

  Drawing happens only when a frame arrives or a setting changes. On this development Mac at 2016×1512 a frame costs about 0.5–1 ms, and about 3 ms with the CRT shader. A Canvas2D path is kept as a fallback.
- **No special server headers are needed.** The app is static and works on any HTTPS host. The service worker makes it available offline.

## Building and running it yourself

**Prerequisites:**
- [Rust](https://rustup.rs) with the WebAssembly target: `rustup target add wasm32-unknown-unknown`.
- `wasm-bindgen-cli` matching the pinned library version: `cargo install wasm-bindgen-cli --version 0.2.126`.
- [Node.js](https://nodejs.org) 20 or newer.

```sh
git clone https://github.com/SPD13/dune-hybrid.git
cd dune-hybrid/web
npm install
npm run wasm        # compile the emulator to WebAssembly (web/src/pkg)
npm run dev         # http://localhost:5174
```

**Production build:**

```sh
npm run build       # -> web/dist (static files)
npm run preview     # serve dist at http://localhost:4174
```

- **Hosting:** copy `web/dist` to any static host (GitHub Pages, Netlify, Cloudflare Pages, nginx…).
- **HTTPS required:** browsers only allow storage and service workers on HTTPS (or `localhost`).

## Development

### Layout

| Path | What |
|---|---|
| `crates/cpu` | 80286 interpreter and its hardware test harness |
| `crates/pc` | The PC: devices, BIOS/DOS services, sound, snapshots, battery saver |
| `crates/runner` | `dune-run`: headless native runner for development and regression checks |
| `crates/web` | WebAssembly bindings (`Emu`) used by the worker |
| `web/` | The web app (Vite + TypeScript) |
| `tools/` | Helpers: CPU test-data conversion (`sst-compact.py`), song lengths from the game's music data (`song-length.py`) |

### Headless runner

```sh
cargo build --release
./target/release/dune-run --dir path/to/game --seconds 60 --shot-every 4 --out out \
    --cmd "ADP330 SBP2227 ENG" --wav out/audio.wav \
    --events "20:key:01:1,20.2:key:01:0,24:mouse:160:182:0,28:mouse:165:182:1,28.1:mouse:165:182:0"
```

- **What it does:** runs the game for the given virtual time, as fast as the machine allows (about 4–18× real time). It writes `out/shot-NNNN.png` screenshots, an optional WAV, and a diagnostics summary.
- **Saves** go to `out/saves`; the game folder is never written to.
- **Scripted input:** `time:key:SCANCODE(hex):1|0` and `time:mouse:X:Y:BUTTONS`, in virtual seconds.
- **Environment switches:**
  - `TRACE_PORTS=1` logs sound/DMA/PIC port traffic, plus the code address of unknown port writes;
  - `TRACE_MOUSE=1` logs mouse-driver calls;
  - `NO_IDLE_SKIP=1` disables the battery saver;
  - `TRACE_MUSIC=1` logs the game's music calls (play/stop/fade, with the song identified) and voice activity;
  - `TRACE_READS=1` logs which DUNE.DAT resources are read and when;
  - `MUSIC_ONLY=1` mutes voices in the recorded WAV;
  - `REPLACED_SONGS=<hex mask>` mutes the FM rendition of those songs, as the web app does when the remaster plays.

### Tests

**Unit tests:**

```sh
cargo test --release
```

**Snapshot round trip on the real game.** Needs the game files: set `DUNE_DIR`, otherwise it looks in `../Cryogenic/dune`.

```sh
cargo test -p pc --release -- --ignored snapshot
```

**CPU against hardware captures** (one-time data download):

```sh
cd tests-data
git clone --depth 1 --filter=blob:none --sparse https://github.com/SingleStepTests/80286.git ss286
(cd ss286 && git sparse-checkout set v1_real_mode)
curl -O https://raw.githubusercontent.com/SingleStepTests/80286/main/tools/moo2json.py
python3 ../tools/sst-compact.py
cd .. && cargo test -p cpu --release -- --ignored single_step
```

**Automated browser runs:** in development, `http://localhost:5174/?devfiles` imports the game files from `../../Cryogenic/dune` (or `$DUNE_DIR`) through a route that exists only on the dev server. Game files are never part of a build.

### Reference emulator

Behaviour is compared against **Cryogenic/Spice86** (OpenRakis), the reference emulator. Run it headless with its MCP server:

```sh
dotnet Cryogenic.dll -e DNCDPRG.EXE -p 4096 --UseCodeOverride true -h Minimal
```

The MCP server is then at `localhost:8081`; its screenshot tool returns the image's file path in `structuredContent.FilePath`.

### Clean-room policy

This engine is written from three sources only:
- the annotated disassembly (`doc/DNCDPRG.lst` in Cryogenic);
- observation of the running original;
- Cryogenic's Apache-2.0 sources.

Do **not** consult the game-logic code of other unlicensed reimplementations while working on this repository.

## Roadmap

- [x] **M1:** the original program boots and plays in the browser.
- [x] **M2:** OPL3 music, Sound Blaster voices, persistent saves.
- [x] **M3:** mobile and comfort, covering:
  - touch controls and on-screen buttons;
  - responsive layout and installable app;
  - snapshots (save anywhere, Continue);
  - battery saver;
  - language, volume and display options;
  - ISO/ZIP import.
- [x] **Picture filters:** WebGL2 pipeline with pixel-art upscalers, sharp/smooth scaling and CRT effects, all switchable live.
- [ ] **HD sprites and text:** draw the game's sprites and font at high resolution where the originals go, by recording the game's drawing operations. First with algorithmic upscaling, then with an optional **HD asset pack** that each player generates on their own computer from their `DUNE.DAT` with an offline tool, and imports like the soundtrack.
- [ ] **Input recorder:** deterministic replays, for bug reports and as a test corpus.
- [x] **Remastered soundtrack** support (the composer's 2024 remaster, purchased by the player), in sync with the game.
- [ ] **MT-32 / General MIDI music** via an emulated MPU-401 and a host synthesizer.
- [ ] **Override mechanism:** replace original routines with Rust ports, verified in-process against the original. This is the path to HD text and graphics, replacement soundtracks and, eventually, a native engine.
- [ ] Firefox and Safari testing on real devices.

## Credits and licenses

- **Built with [Claude Code](https://claude.com/claude-code):** this project was created with Claude Code, Anthropic's agentic coding tool, working with the project's maintainer. That covers the emulator, the CPU validation, the sound hardware, the web app and this documentation.
- **This project:** Apache License 2.0 (see [LICENSE](LICENSE) and [NOTICE](NOTICE)).
- **[OpenRakis/Cryogenic](https://github.com/OpenRakis/Cryogenic)** and **[Spice86](https://github.com/OpenRakis/Spice86)** (Apache-2.0): the reference emulator, documentation of the game's drivers, and the annotated disassembly (from madmoose's `dune-chani` annotations).
- **Music** by **Stéphane Picq**. The optional remastered soundtrack is *Dune Spice Opera 2024 remaster* by EXXOS (Stéphane Picq with Philippe Ulrich), sold separately at <https://stphanepicq.bandcamp.com/album/dune-spice-opera-2024-remaster-lp> and not included here.
- **[oplon](https://codeberg.org/sbechet/oplon)** (MIT): OPL2/OPL3 FM synthesis.
- **Picture filter shaders** (ported to WebGL2; details in [NOTICE](NOTICE)):
  - **MMPX** by Morgan McGuire and Mara Gagiu (MIT), from hunterk's GLSL adaptation;
  - **xBR-lv2** by Hyllian (MIT);
  - **CRT shader** by Timothy Lottes (public domain);
  - sharp-bilinear after Themaister (public domain);
  - Scale2x/EPX after Andrea Mazzoleni's algorithm (own implementation).
- **[miniz_oxide](https://github.com/Frommi/miniz_oxide)** (MIT/Apache-2.0/Zlib): snapshot compression.
- **[SingleStepTests/80286](https://github.com/SingleStepTests/80286)** (MIT): CPU test vectors, downloaded at test time and not redistributed.
- ***Dune*** (1992) © Cryo Interactive Entertainment / Virgin Games; based on the novel by Frank Herbert. No game data is included in this repository.
