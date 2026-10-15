"""Dune HD Studio (prototype): review generated HD art, set up the generation
environment and models, and run generations, with progress.

    python server.py WORK_DIR [--dat DUNE.DAT] [--dune-hd PATH] [--env DIR] [--port 8765]

WORK_DIR holds `review/items.json` (what to show; `dune-hd inventory` writes
it), `versions/<item id, / as __>/*.png` (versions dropped by the generation
tools, each with an optional .json sidecar: label, settings, default) and
receives `review/review.json` (statuses, preferred versions, notes and boxes),
which the generation tools and Claude read. `WORK_DIR/studio.json` remembers
the options. Only this computer can connect (127.0.0.1), and nothing leaves
it: the pictures are derived from your copy of the game.

The generation environment (Python with PyTorch built for this machine's GPU:
CUDA on NVIDIA, Apple's GPU on a Mac, else CPU) goes to the studio's own
folder (see system.data_dir) unless --env names one; models go to the Hugging
Face cache.
"""
import argparse
import json
import os
import shutil
import sys
import tempfile
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, unquote, urlparse

import jobs
import system

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
TOOLS = os.path.join(REPO, "tools", "portraits")
TYPES = {".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".png": "image/png",
         ".gif": "image/gif", ".jpg": "image/jpeg", ".json": "application/json"}


def version_dir(work, item_id):
    """Where generation tools drop an item's versions: WORK/versions/<id with / as __>/."""
    return os.path.join(work, "versions", item_id.replace("/", "__"))


def attach_versions(work, items):
    """Add the pictures found in each item's version folder (NAME.png, with an
    optional NAME.json: {"label", "settings", "default": true})."""
    for it in items.get("items", []):
        d = version_dir(work, it["id"])
        if not os.path.isdir(d):
            continue
        known = {v["path"] for v in it.get("versions", [])}
        pngs = [n for n in os.listdir(d) if n.endswith(".png")]
        # Oldest first, so the newest version is last in the pickers (batch names are hashes).
        for name in sorted(pngs, key=lambda n: (os.path.getmtime(os.path.join(d, n)), n)):
            rel = os.path.relpath(os.path.join(d, name), work).replace(os.sep, "/")
            if rel in known:
                continue
            meta = {}
            side = os.path.join(d, name[:-4] + ".json")
            if os.path.exists(side):
                try:
                    meta = json.load(open(side))
                except ValueError:
                    pass
            v = {"id": name[:-4], "label": meta.get("label", name[:-4]), "path": rel, "settings": meta.get("settings", {})}
            it.setdefault("versions", []).append(v)
            if meta.get("default") or not it.get("default"):
                it["default"] = v["id"]


class Studio:
    def __init__(self, work, cfg):
        self.work, self.cfg = work, cfg
        self.queue = jobs.Queue()
        self.recipes = cfg.get("recipes") or os.path.join(REPO, "recipes")
        self.env = cfg.get("env") or os.path.join(system.data_dir(), "env")
        self.hw = system.hardware()
        self.env_info = {"ok": False, "checking": True}
        self.recheck_env()

    def recheck_env(self, *_):
        self.env_info = {**self.env_info, "checking": True}

        def run():
            self.env_info = {**system.check_env(self.env), "checking": False}
        threading.Thread(target=run, daemon=True).start()

    def system(self):
        models = system.model_status(os.path.join(self.recipes, "models.json"))
        need_models = sum(m["size_gb"] for m in models if not m["cached"] and not m.get("optional"))
        return {
            "hardware": self.hw, "env": {"path": self.env, **self.env_info}, "models": models,
            "disk": {"free_gb": system.free_gb(self.env), "env_gb": system.ENV_SIZE_GB[self.hw["kind"]], "models_gb": round(need_models, 1),
                     "hf_cache": system.hf_hub_dir()},
            "dat": self.cfg.get("dat"), "dat_ok": bool(self.cfg.get("dat") and os.path.isfile(self.cfg["dat"])),
            "dune_hd": self.cfg.get("dune_hd"), "dune_hd_ok": bool(self.cfg.get("dune_hd") and os.path.isfile(self.cfg["dune_hd"])),
            "work": self.work, "recipes": self.recipes, "data_dir": system.data_dir(),
        }

    def ready(self):
        """Why generation cannot run yet, or None."""
        if not self.env_info.get("ok"):
            return "the generation environment is not installed (Setup)"
        if not all(m["cached"] for m in system.model_status(os.path.join(self.recipes, "models.json")) if not m.get("optional")):
            return "the models are not downloaded (Setup)"
        return None

    def setup(self, what):
        title = {"env": "Install the generation environment", "models": "Download the models"}[what]
        if self.queue.busy(title):
            raise ValueError("already queued")
        if what == "env":
            need, free = system.ENV_SIZE_GB[self.hw["kind"]], system.free_gb(self.env)
            if free < need + 1:
                raise ValueError(f"not enough disk space: {free} GB free, about {need + 1:.0f} GB needed")
            if os.path.exists(self.env):
                # Only an environment the studio made in its own folder is replaced.
                if os.path.realpath(self.env).startswith(os.path.realpath(system.data_dir()) + os.sep):
                    shutil.rmtree(self.env)
                else:
                    raise ValueError(f"{self.env} was given with --env: the studio does not replace it")
            steps = system.env_steps(self.env, self.hw["kind"], os.path.join(TOOLS, "requirements-ml.txt"))
            return self.queue.add(jobs.Job(title, steps, on_done=self.recheck_env))
        if not self.env_info.get("ok"):
            raise ValueError("install the generation environment first")
        models = [m for m in system.model_status(os.path.join(self.recipes, "models.json")) if not m.get("optional")]
        need, free = sum(m["size_gb"] for m in models if not m["cached"]), system.free_gb(system.hf_hub_dir())
        if free < need + 1:
            raise ValueError(f"not enough disk space: {free} GB free, {need + 1:.0f} GB needed")
        cmd = [system.env_python(self.env), os.path.join(TOOLS, "fetch_models.py"), "--models", os.path.join(self.recipes, "models.json"),
               "--only", ",".join(m["id"] for m in models if not m["cached"])]
        return self.queue.add(jobs.Job(title, [("download the models", cmd)]))

    def recipe(self, item_id):
        """The item's current settings (hdgen.py recipe), for Try settings."""
        import subprocess
        if not self.env_info.get("ok"):
            raise ValueError("install the generation environment first (Setup)")
        out = subprocess.run([system.env_python(self.env), os.path.join(TOOLS, "hdgen.py"), "recipe", self.work, "--id", item_id,
                              "--recipes", self.recipes], capture_output=True, text=True, timeout=60)
        if out.returncode:
            raise ValueError(out.stderr.strip().splitlines()[-1] if out.stderr.strip() else "hdgen.py recipe failed")
        return json.loads(out.stdout.strip().splitlines()[-1])

    def generate(self, item_id, override=None):
        why = self.ready()
        if why:
            raise ValueError(why)
        kind, _, rest = item_id.partition("/")
        heads = json.load(open(os.path.join(self.recipes, "heads.json")))["characters"]
        name, _, fid = rest.partition("/")
        if kind != "head" or name not in heads:
            # Every other item: hdgen with its recipe or its class's defaults; with Try settings on top.
            py = system.env_python(self.env)
            args = batch_args(self, {"ids": [item_id]})
            title = f"Generate {item_id}"
            steps = []
            if override:
                label = "try: " + ", ".join(try_label(k, v) for k, v in override.items())
                args += ["--override", json.dumps(override), "--label", label]
                title = f"Try {item_id}"
                # A base model picked for the try and not downloaded yet: fetched first.
                m = next((m for m in system.model_status(os.path.join(self.recipes, "models.json")) if m["id"] == override.get("model")), None)
                if m and not m["cached"]:
                    steps.append((f"download {m['repo']} ({m['size_gb']} GB)", [py, os.path.join(TOOLS, "fetch_models.py"), "--models",
                                                                               os.path.join(self.recipes, "models.json"), "--only", m["id"]]))
            steps.append(("generate", [py] + args))
            return self.queue.add(jobs.Job(title, steps, env={"HF_HUB_OFFLINE": "1"}))
        rec = heads[name]
        if fid != rec.get("base", "f00") and fid not in rec.get("variants", {}):
            raise ValueError(f"{name} {fid} is not in its recipe yet (recipes/heads.json variants): add a note and Claude extends it")
        steps = []
        heads_dir = os.path.join(self.work, "heads")
        if not os.path.exists(os.path.join(heads_dir, f"{name}-f00.png")):
            if not (self.cfg.get("dat") and self.cfg.get("dune_hd")):
                raise ValueError("start the studio with --dat and --dune-hd to extract the heads")
            steps.append(("extract the talking heads", [self.cfg["dune_hd"], "heads", "--dat", self.cfg["dat"], "--work", self.work]))
        py = system.env_python(self.env)
        cmd = [py, os.path.join(TOOLS, "generate.py"), name, "--heads", heads_dir, "--characters", os.path.join(self.recipes, "heads.json"),
               "--out", os.path.join(self.work, "gen", name), "--only", fid, "--publish", self.work]
        title = f"Generate {name} {fid}"
        if override:
            # A try on a recipe head: its own render folder (the recipe's cache stays), a label.
            flags = {"strength": "--strength", "cond": "--cond", "soften": "--soften", "seed": "--seed", "model": "--model", "prompt": "--prompt"}
            unknown = set(override) - set(flags)
            if unknown:
                raise ValueError(f"{name}: these settings do not apply to talking heads: {', '.join(sorted(unknown))}")
            cmd[cmd.index("--out") + 1] = os.path.join(self.work, "gen", f"{name}-try")
            for k, v in override.items():
                cmd += [flags[k], str(v)]
            cmd += ["--force", "--label", "try: " + ", ".join(try_label(k, v) for k, v in override.items())]
            title = f"Try {name} {fid}"
            m = next((m for m in system.model_status(os.path.join(self.recipes, "models.json")) if m["id"] == override.get("model")), None)
            if m and not m["cached"]:
                steps.append((f"download {m['repo']} ({m['size_gb']} GB)", [py, os.path.join(TOOLS, "fetch_models.py"), "--models",
                                                                           os.path.join(self.recipes, "models.json"), "--only", m["id"]]))
        steps.append((f"generate {name} {fid}", cmd))
        return self.queue.add(jobs.Job(title, steps, env={"HF_HUB_OFFLINE": "1"}))


def try_label(k, v):
    names = {"strength": "strength", "cond": "control", "soften": "blur", "seed": "seed", "model": "model"}
    if k == "prompt":
        return "own prompt"
    if k == "negative":
        return "own negatives"
    if k == "detail":
        return "detail off" if v is None else f"detail {v.get('strength')}"
    return f"{names.get(k, k)} {v}"


def batch_args(studio, data):
    """hdgen.py batch arguments for a batch request: classes, or the ids of the
    items shown by the studio's filter; redo items whose version is up to date."""
    args = [os.path.join(TOOLS, "hdgen.py"), "batch", studio.work, "--recipes", studio.recipes]
    if data.get("ids"):
        # One file per job: a queued job must not read the next one's list.
        d = os.path.join(studio.work, "review", "jobs")
        os.makedirs(d, exist_ok=True)
        fd, path = tempfile.mkstemp(dir=d, prefix="ids-", suffix=".json")
        with os.fdopen(fd, "w") as f:
            json.dump(data["ids"], f)
        args += ["--ids", path]
    elif data.get("classes"):
        args += ["--classes", ",".join(data["classes"])]
    if data.get("redo"):
        args.append("--redo")
    if data.get("limit"):
        args += ["--limit", str(int(data["limit"]))]
    if studio.cfg.get("dune_hd") and studio.cfg.get("dat"):
        args += ["--dune-hd", studio.cfg["dune_hd"], "--dat", studio.cfg["dat"]]
    return args


def make_handler(studio):
    work = studio.work
    review_dir = os.path.join(work, "review")
    items_path = os.path.join(review_dir, "items.json")
    review_path = os.path.join(review_dir, "review.json")
    resolutions_path = os.path.join(review_dir, "resolutions.json")  # boxes acted on (resolutions.py)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args):
            pass

        def send(self, code, body, ctype="application/json"):
            if isinstance(body, (dict, list)):
                body = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def file(self, path):
            if not os.path.isfile(path):
                return self.send(404, b"not found", "text/plain")
            with open(path, "rb") as f:
                self.send(200, f.read(), TYPES.get(os.path.splitext(path)[1].lower(), "application/octet-stream"))

        def do_GET(self):
            path = unquote(urlparse(self.path).path)
            if path in ("/", "/index.html"):
                return self.file(os.path.join(HERE, "index.html"))
            if path == "/api/items":
                items = json.load(open(items_path)) if os.path.exists(items_path) else {"items": []}
                attach_versions(work, items)
                review = json.load(open(review_path)) if os.path.exists(review_path) else {"assets": {}}
                resolutions = json.load(open(resolutions_path, encoding="utf-8")) if os.path.exists(resolutions_path) else {"assets": {}}
                return self.send(200, {"items": items, "review": review, "resolutions": resolutions})
            if path == "/api/system":
                return self.send(200, studio.system())
            if path == "/api/jobs":
                return self.send(200, [j.view() for j in studio.queue.jobs[-30:]])
            if path == "/api/recipe":
                try:
                    return self.send(200, studio.recipe(parse_qs(urlparse(self.path).query).get("id", [""])[0]))
                except (ValueError, OSError, StopIteration) as e:
                    return self.send(400, {"error": str(e)})
            if path.startswith("/files/"):
                # Only files inside WORK_DIR.
                full = os.path.realpath(os.path.join(work, path[len("/files/"):]))
                if not full.startswith(os.path.realpath(work) + os.sep):
                    return self.send(403, b"forbidden", "text/plain")
                return self.file(full)
            self.send(404, b"not found", "text/plain")

        def do_POST(self):
            path = urlparse(self.path).path
            body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
            try:
                data = json.loads(body or b"{}")
            except ValueError:
                return self.send(400, b"bad json", "text/plain")
            try:
                if path == "/api/review":
                    os.makedirs(review_dir, exist_ok=True)
                    # Written atomically: the generation tools may read it at any time.
                    fd, tmp = tempfile.mkstemp(dir=review_dir, suffix=".tmp")
                    with os.fdopen(fd, "w") as f:
                        json.dump(data, f, indent=2, ensure_ascii=False)
                    os.replace(tmp, review_path)
                    return self.send(200, {"ok": True})
                if path == "/api/setup":
                    return self.send(200, studio.setup(data.get("what")).view())
                if path == "/api/batch/plan":
                    if not studio.env_info.get("ok"):
                        raise ValueError("install the generation environment first (Setup)")
                    import subprocess
                    out = subprocess.run([system.env_python(studio.env)] + batch_args(studio, data) + ["--plan", "--json"],
                                         capture_output=True, text=True, timeout=120)
                    plan = json.loads(out.stdout.strip().splitlines()[-1])
                    defaults = json.load(open(os.path.join(studio.recipes, "defaults.json")))
                    plan["seconds_each"] = defaults["seconds_per_generation"][studio.hw["kind"]]
                    return self.send(200, plan)
                if path == "/api/batch":
                    why = studio.ready()
                    if why:
                        raise ValueError(why)
                    py = system.env_python(studio.env)
                    what = f"{len(data['ids'])} filtered items" if data.get("ids") else ", ".join(data.get("classes") or ["default classes"])
                    job = jobs.Job(f"Batch: {what}", [("generate", [py] + batch_args(studio, data))], env={"HF_HUB_OFFLINE": "1"})
                    return self.send(200, studio.queue.add(job).view())
                if path == "/api/generate":
                    return self.send(200, studio.generate(data.get("item", ""), data.get("override")).view())
                if path == "/api/jobs/cancel":
                    job = studio.queue.get(int(data.get("id", 0)))
                    if job:
                        job.cancel()
                    return self.send(200, {"ok": bool(job)})
            except (ValueError, KeyError, OSError) as e:
                return self.send(400, {"error": str(e)})
            self.send(404, b"not found", "text/plain")

    return Handler


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("work")
    ap.add_argument("--dat", help="your DUNE.DAT")
    ap.add_argument("--dune-hd", help="the dune-hd program (target/release/dune-hd)")
    ap.add_argument("--env", help="use this Python environment for generation instead of installing one")
    ap.add_argument("--recipes", help="recipe bundle (default: the repository's recipes/)")
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--no-browser", action="store_true")
    a = ap.parse_args()
    work = os.path.abspath(a.work)
    if not os.path.isdir(work):
        sys.exit(f"{work}: not a folder")
    cfg_path = os.path.join(work, "studio.json")
    cfg = json.load(open(cfg_path)) if os.path.exists(cfg_path) else {}
    for k in ("dat", "dune_hd", "env", "recipes"):
        v = getattr(a, k)
        if v:
            cfg[k] = os.path.abspath(v)
    json.dump(cfg, open(cfg_path, "w"), indent=2)
    studio = Studio(work, cfg)
    server = ThreadingHTTPServer(("127.0.0.1", a.port), make_handler(studio))
    url = f"http://127.0.0.1:{a.port}/"
    print(f"Dune HD Studio on {url} (work folder {work}; {studio.hw['label']}); Ctrl+C to stop", flush=True)
    if not a.no_browser:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
