# Dune HD Studio (prototype)

A local app to review the game's art and its generated HD versions, set up the generation environment and the models, and run generations with progress. It is the prototype of the studio planned in [docs/hd-studio-plan.md](../../docs/hd-studio-plan.md); the final one will be a single Rust program.

```sh
cargo build --release
# 1. Every asset of your DUNE.DAT as review units, in true colours:
./target/release/dune-run --dir path/to/game --seconds 90 --events "61.5:key:1:1,61.6:key:1:0" --out /tmp/run --rooms WORK/rooms --gfx-trace WORK/traces/intro.jsonl
./target/release/dune-hd inventory --dat path/to/DUNE.DAT --work WORK --recipes recipes --rooms WORK/rooms --trace WORK/traces/intro.jsonl
# 2. The studio (Python 3, standard library only):
python3 tools/studio/server.py WORK --dat path/to/DUNE.DAT --dune-hd target/release/dune-hd
```

It opens http://127.0.0.1:8765/. Only this computer can connect, and nothing leaves it: the pictures are derived from your copy of the game. The options are remembered in `WORK/studio.json`.

## Setup: environment, models, GPU

**Setup** shows the game file, `dune-hd`, the graphics card, the generation environment and the models, with free disk space:
- **Graphics card:** found with `nvidia-smi` (an NVIDIA GPU, its memory and driver), or Apple's GPU on a Mac, else none (CPU only, hours per picture).
- **Install** creates a Python environment in the studio's own folder:
  - `%LOCALAPPDATA%\DuneHD\env` on Windows;
  - `~/.local/share/dune-hd/env` on Linux;
  - `~/Library/Application Support/DuneHD/env` on macOS.

  It installs PyTorch built for the machine: the CUDA build on an NVIDIA GPU, where only the driver is needed since the wheels bring CUDA; the default build on a Mac; the CPU build otherwise. Then it installs the generation libraries (`tools/portraits/requirements-ml.txt`). It uses `uv` when installed, else `venv` and `pip`. `--env DIR` uses an existing environment instead; the studio never replaces an environment it didn't make.
- **Download** fetches only the files the models need (`recipes/models.json`, about 9 GB) into the Hugging Face cache.

Generation picks the device by itself (`tools/portraits/device.py`):
- **NVIDIA GPU:** fp16. With 12 GB or more it runs with no memory saving (fastest); with 8–12 GB it slices attention; under 8 GB it offloads the model part by part.
- **Apple GPU:** fp16 with attention slicing.
- **CPU:** fp32.

`DUNE_HD_DEVICE=cuda|mps|cpu` forces one.

## Reviewing

- **Finding items:**
  - the sidebar groups items by class and sheet, with approved/total counts;
  - search, and filter by status, "with generated versions", "with boxes" or "colours not certain" (⚠: the colours of a sprite whose palette the inventory could not establish);
  - **Grid** (<kbd>G</kbd>) shows thumbnails.
- **Comparing:** side by side, slider, flip (<kbd>Space</kbd>), and **detail map** (<kbd>D</kbd>: where a version adds fine detail, cyan to yellow). Zoom with the wheel; <kbd>F</kbd>, <kbd>1</kbd> and <kbd>2</kbd> fit, show 1:1 and 2:1.
- **Annotating:**
  - **boxes** (<kbd>B</kbd>, then drag) of kind *part to describe*, *problem*, *keep original* or *note*, each with a text;
  - a status, a preferred version, and notes.

  Everything saves to `WORK/review/review.json`.
- **Generate a new version** runs the item's recipe as a job. Its progress shows under **Jobs**, and the result appears as a new version. Talking heads can be generated so far; scenes, rooms and the interface come next.

## Talking heads: frames and talking groups

A head is listed by the frames the game lists (`head/NAME/fNN`) and by its **talking groups** (`head/NAME/vGG`): each mouth or eyes group drawn over frame 0. While a character talks, the game assembles its face from frame 0 and such groups, as each sentence's lip-sync data says, so every group needs a version for the face to stay generated in the game. Groups that build the faces listed as frames (the Fremen sheets' hair and beards) are reviewed in those frames instead.

Talking groups are made from the head's frame 0 with its recipe (`generate.py`: the same generation, same seed, kept only where the group differs). A head without a recipe in `recipes/heads.json` cannot have its groups generated yet.

## Building a pack

**Pack** builds an HD pack for Dune Hybrid from the chosen versions:
1. `export_pack.py` takes each item's preferred version, else its default. That's every approved item, or every item with a version except rejected ones (work in progress). It writes them under the names `dune-hd build --art` expects, with their palette (`WORK/inventory/palettes`, from `dune-hd inventory`) and, for characters, true colour.
2. `dune-hd build` makes `WORK/packs/dune-hd-pack-<scope>-<time>.zip`. The rest of the art gets the built-in upscale, and talking heads become head plates in true colour.

The dialog lists the built packs, with what was left out and why. Import a pack in Dune Hybrid's start page (HD art pack), or try it headless with `dune-run --hd 4 --hd-shots --hd-pack PACK.zip`. Packs are made from your copy of the game: keep them for yourself.

## The loop with Claude

1. You review in the studio: boxes, notes, statuses.
2. Claude reads them: `python3 tools/studio/review_todo.py WORK` lists what to act on, with each box in the original's pixels.
3. Claude updates the recipes in [`recipes/`](../../recipes), the bundle that ships for one-click generation, and runs the generation CLI. Those are the same commands the studio's jobs run, and they publish into `WORK/versions/`.
4. The new versions appear in the studio for the next review.

## Files

- **`WORK/review/items.json`** (from `dune-hd inventory`): one item per review unit. Each has an `id`, `title`, `class`, `group`, `kind`, `size`, `palette {source, ok}` and `art_name` (the name `dune-hd build --art` expects), plus the `original` and any `versions`.
- **`WORK/versions/<id, / as __>/NAME.png`**: generated versions, each with an optional `NAME.json` (`label`, `settings`, `default`). The studio attaches them to their item.
- **`WORK/review/review.json`**: the review.
- **`WORK/packs/`**: built packs, and the art exported for each (`art-<time>/`, with `export.json`).
- **`server.py`** (HTTP and API), **`jobs.py`** (the job queue and progress), **`system.py`** (GPU, environment, models, disk), **`export_pack.py`** (the pack export), **`index.html`** (the page).
