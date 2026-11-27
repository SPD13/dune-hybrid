"""The studio's packs: variations of the HD pack made from the same game, e.g. with
another base model. WORK/packs.json lists them and names the active one:

    {"active": "main", "packs": [{"id": "main", "title": "Main", "model": null}, ...]}

Each pack has its own recipes, since another model needs other fixes: the main pack
the repository's recipes/ (the bundle that ships), every other pack recipes/packs/<id>/,
a copy of the recipes of the pack it was made from, with its base model ("model", an
id from models.json) written into it (models.json, heads.json's defaults). Its own
recipes can still give a character or a class another model.
The main pack's review and renders are the work folder's as they always were
(WORK/review/review.json, WORK/gen); every other pack's are in WORK/variants/<id>/
(review.json, gen/). All packs' versions live in WORK/versions; a version made for a
pack other than main says so in its sidecar ("pack": id), and each pack shows only
its own.

    python packs.py WORK                  the packs, the active one marked, with their recipes
    python packs.py run WORK ID -- CMD…   run CMD, then tag the versions it made with ID
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

MAIN = "main"
RECIPES = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "recipes"))


def path(work):
    return os.path.join(work, "packs.json")


def load(work):
    try:
        cfg = json.load(open(path(work), encoding="utf-8"))
    except (OSError, ValueError):
        cfg = {}
    packs = cfg.get("packs") or []
    if not any(p["id"] == MAIN for p in packs):
        packs.insert(0, {"id": MAIN, "title": "Main", "model": None})
    active = cfg.get("active") if any(p["id"] == cfg.get("active") for p in packs) else MAIN
    return {"active": active, "packs": packs}


def save(work, cfg):
    tmp = path(work) + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(cfg, f, indent=2, ensure_ascii=False)
    os.replace(tmp, path(work))


def get(work, pid=None):
    cfg = load(work)
    pid = pid or cfg["active"]
    return next(p for p in cfg["packs"] if p["id"] == pid)


def folder(work, pid):
    return work if pid == MAIN else os.path.join(work, "variants", pid)


def review_path(work, pid):
    return os.path.join(work, "review", "review.json") if pid == MAIN else os.path.join(folder(work, pid), "review.json")


def gen_dir(work, pid):
    return os.path.join(folder(work, pid), "gen")


def recipes_dir(pid, base=None):
    """The pack's recipe bundle: base (the repository's recipes/) for main, else base/packs/<id>
    (a pack imported from a built pack has none of its own: main's)."""
    base = base or RECIPES
    own = os.path.join(base, "packs", pid)
    return own if pid != MAIN and os.path.isdir(own) else base


def copy_recipes(src, dst, model=None):
    """The bundle at src copied to dst (not its packs/), with `model` (an id from models.json)
    as the base model of every class and rule, and the talking heads' default."""
    shutil.copytree(src, dst, ignore=lambda d, names: [n for n in names if os.path.normpath(d) == os.path.normpath(src) and n == "packs"
                                                       or n.endswith((".png", ".tmp"))])
    if not model:
        return
    mp = os.path.join(dst, "models.json")
    m = json.load(open(mp, encoding="utf-8"))
    repo = next((d["repo"] for d in m["downloads"] if d["id"] == model), None)
    if not repo:
        raise ValueError(f"{model} is not in models.json")
    base_ids = {d["id"] for d in m["downloads"] if (d.get("role") or "").startswith("base model")}
    for k, ids in m.get("classes", {}).items():
        m["classes"][k] = [model if i in base_ids else i for i in ids]
    for r in m.get("use", {}).get("rules", []):
        r["model"] = model
    with open(mp, "w", encoding="utf-8", newline="\n") as f:
        json.dump(m, f, indent=2, ensure_ascii=False)
        f.write("\n")
    hp = os.path.join(dst, "heads.json")
    if os.path.exists(hp):
        # Textually, so the copy keeps heads.json's layout (its diff against main stays readable).
        text = open(hp, encoding="utf-8", newline="").read()
        at = text.index('"defaults"')
        text = text[:at] + re.sub(r'"model": "[^"]*"', f'"model": "{repo}"', text[at:], count=1)
        open(hp, "w", encoding="utf-8", newline="").write(text)


def version_pack(meta):
    return meta.get("pack") or MAIN


def select(work, pid):
    cfg = load(work)
    if not any(p["id"] == pid for p in cfg["packs"]):
        raise ValueError(f"no pack {pid}")
    cfg["active"] = pid
    save(work, cfg)
    return cfg


def add(work, title, model=None, copy_from=None, base=None, imported=None):
    """A new pack, made active. Its recipes are a copy of copy_from's with its model;
    its review starts from copy_from's notes, boxes and open-mouth ticks, every item
    back to new. An imported pack (imported: what it was made from, see import_pack.py)
    has no recipes and starts with an empty review."""
    cfg = load(work)
    title = (title or "").strip()
    if not title:
        raise ValueError("give the pack a name")
    slug = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-") or "pack"
    pid, n = slug, 2
    while any(p["id"] == pid for p in cfg["packs"]):
        pid, n = f"{slug}-{n}", n + 1
    src_pack = copy_from or cfg["active"]
    dst = os.path.join(base or RECIPES, "packs", pid)
    if os.path.exists(dst):
        raise ValueError(f"{dst} already exists")
    if not imported:
        copy_recipes(recipes_dir(src_pack, base), dst, model)
    review = {"version": 1, "assets": {}}
    src = review_path(work, src_pack)
    if os.path.exists(src) and not imported:
        old = json.load(open(src, encoding="utf-8"))
        for k, a in old.get("assets", {}).items():
            keep = {f: a[f] for f in ("notes", "boxes", "open_mouth") if f in a}
            if keep.get("notes") or keep.get("boxes") or "open_mouth" in keep:
                review["assets"][k] = {"status": "new", "preferred": None, "notes": "", "boxes": [], **keep}
    os.makedirs(folder(work, pid), exist_ok=True)
    with open(review_path(work, pid), "w", encoding="utf-8") as f:
        json.dump(review, f, indent=2, ensure_ascii=False)
    entry = {"id": pid, "title": title, "model": model or None, "created": time.strftime("%Y-%m-%d %H:%M")}
    entry.update({"imported": imported} if imported else {"from": src_pack})
    cfg["packs"].append(entry)
    cfg["active"] = pid
    save(work, cfg)
    return cfg


def delete(work, pid, base=None):
    """Removes a pack from the list. Its review, render cache and recipes move to
    WORK/variants/deleted/; its versions stay in WORK/versions (tagged, never shown)."""
    if pid == MAIN:
        raise ValueError("the main pack can't be deleted")
    cfg = load(work)
    if not any(p["id"] == pid for p in cfg["packs"]):
        raise ValueError(f"no pack {pid}")
    cfg["packs"] = [p for p in cfg["packs"] if p["id"] != pid]
    if cfg["active"] == pid:
        cfg["active"] = MAIN
    trash = os.path.join(work, "variants", "deleted", f"{pid}-{time.strftime('%Y%m%d-%H%M%S')}")
    d = folder(work, pid)
    if os.path.isdir(d):
        os.makedirs(os.path.dirname(trash), exist_ok=True)
        shutil.move(d, trash)
    r = os.path.join(base or RECIPES, "packs", pid)  # its own only, never main's
    if os.path.isdir(r):
        os.makedirs(trash, exist_ok=True)
        shutil.move(r, os.path.join(trash, "recipes"))
    save(work, cfg)
    return cfg


def tag(work, pid, since):
    """Marks the versions written since `since` (sidecars without a pack) as pid's."""
    root = os.path.join(work, "versions")
    n = 0
    for d in os.listdir(root) if os.path.isdir(root) else []:
        for name in os.listdir(os.path.join(root, d)):
            if not name.endswith(".png"):
                continue
            png = os.path.join(root, d, name)
            if os.path.getmtime(png) < since:
                continue
            side = png[:-4] + ".json"
            meta = json.load(open(side)) if os.path.exists(side) else {}
            if "pack" in meta:
                continue
            meta["pack"] = pid
            json.dump(meta, open(side, "w"), indent=2)
            n += 1
    return n


def main():
    if len(sys.argv) >= 4 and sys.argv[1] == "run":
        work, pid = sys.argv[2], sys.argv[3]
        cmd = sys.argv[sys.argv.index("--") + 1:]
        since = time.time() - 1
        code = subprocess.call(cmd)
        if pid != MAIN:
            print(f"{tag(work, pid, since)} versions tagged for pack {pid}", flush=True)
        return code
    work = sys.argv[1] if len(sys.argv) > 1 else "."
    cfg = load(work)
    for p in cfg["packs"]:
        print(("* " if p["id"] == cfg["active"] else "  ") + f"{p['id']}: {p['title']}, " + (f"imported from {p['imported']['zip']}" if p.get("imported") else f"model {p.get('model') or 'as main'}, recipes {recipes_dir(p['id'])}"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
