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

## The loop with Claude

1. You review in the studio: boxes, notes, statuses.
2. Claude reads them: `python3 tools/studio/review_todo.py WORK` lists what to act on, with each box in the original's pixels.
3. Claude updates the recipes in [`recipes/`](../../recipes), the bundle that ships for one-click generation, and runs the generation CLI. Those are the same commands the studio's jobs run, and they publish into `WORK/versions/`.
4. The new versions appear in the studio for the next review.

## Files

- **`WORK/review/items.json`** (from `dune-hd inventory`): one item per review unit. Each has an `id`, `title`, `class`, `group`, `kind`, `size`, `palette {source, ok}` and `art_name` (the name `dune-hd build --art` expects), plus the `original` and any `versions`.
- **`WORK/versions/<id, / as __>/NAME.png`**: generated versions, each with an optional `NAME.json` (`label`, `settings`, `default`). The studio attaches them to their item.
- **`WORK/review/review.json`**: the review.
- **`server.py`** (HTTP and API), **`jobs.py`** (the job queue and progress), **`system.py`** (GPU, environment, models, disk), **`index.html`** (the page).
