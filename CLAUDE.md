# Dune Hybrid: notes for Claude

- **Continuing the generative HD art and studio work?** Read [docs/hd-handoff.md](docs/hd-handoff.md) first, then [docs/hd-studio-plan.md](docs/hd-studio-plan.md).
- **Never commit or share game-derived files:** packs, renders, extracted art, studio work folders, `DUNE.DAT`, saves.
- **Commit or push only when the user asks.**
- **Clean-room rule for engine code:** use only `DNCDPRG.lst`, the running original (Cryogenic/Spice86) and Cryogenic's sources; never dune-re's game logic.
- **The user's annotations from the studio** are in `<work>/review/review.json`. Read them with `python tools/studio/review_todo.py <work>`; turn them into changes in `recipes/` (the bundle that ships), then regenerate with `tools/portraits/hdgen.py` or `generate.py`.
- **Heavy generation runs on the Windows NVIDIA machine, not the Mac.**
