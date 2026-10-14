"""Download the models of recipes/models.json (only the files they need) into
the Hugging Face cache, with progress the studio can read.

    python fetch_models.py [--models recipes/models.json] [--only id,id] [--check]

--check only reports what is already there (JSON on stdout).
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def cached(repo, patterns):
    """True when the model's weights are already in the local cache."""
    from huggingface_hub import snapshot_download
    try:
        path = snapshot_download(repo, allow_patterns=patterns, local_files_only=True)
    except Exception:
        return False
    return any(f.endswith(".safetensors") and os.path.exists(os.path.join(r, f)) for r, _, fs in os.walk(path) for f in fs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", default=os.path.join(HERE, "..", "..", "recipes", "models.json"))
    ap.add_argument("--only")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    models = json.load(open(a.models))["downloads"]
    if a.only:
        models = [m for m in models if m["id"] in a.only.split(",")]
    if a.check:
        print(json.dumps({m["id"]: cached(m["repo"], m["files"]) for m in models}))
        return
    from huggingface_hub import snapshot_download
    total = sum(m["size_gb"] for m in models)
    done = 0.0
    for m in models:
        print(f"@stage downloading {m['repo']} ({m['size_gb']:.1f} GB)", flush=True)
        print(f"@progress {done / total:.3f}", flush=True)
        snapshot_download(m["repo"], allow_patterns=m["files"])
        done += m["size_gb"]
    print("@progress 1.0", flush=True)
    print("models ready", flush=True)


if __name__ == "__main__":
    sys.exit(main())
