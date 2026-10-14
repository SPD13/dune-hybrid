# Recipe bundle

The recipes the studio's one-click mode follows, and that we refine while reviewing: our text and numbers only, never game data. They are versioned with the code; `bundle.json` gives the bundle's version and the `DUNE.DAT` versions it was made for. See [docs/hd-studio-plan.md](../docs/hd-studio-plan.md).

| File | Holds |
|---|---|
| `bundle.json` | Version, supported game versions (`DUNE.DAT` table-of-contents hash), file list |
| `models.json` | The models to download (only the files listed), and which class uses which |
| `assets.json` | How `dune-hd inventory` sorts sheets into classes, and which palette shows sprites whose sheet has none |
| `heads.json` | Talking heads: description, negatives, settings, variants (rerender or recolour), status |
| `scenes.json` | Full-screen pictures: palette, prompt, settings |
| `rooms.json` | Rooms by key: prompt, settings, and parts (people and objects painted into the room, redone in close-up) |
| `interface.json` | Ornamental interface pieces; functional ones stay classic |

An entry's `status` says whether one-click generation may use it (`approved …`) or not yet (`draft`).
