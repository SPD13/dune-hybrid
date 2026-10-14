"""What the studio needs to know about this computer: GPU, the Python
environment for generation, the models, free disk space. Standard library only.
"""
import glob
import json
import os
import platform
import shutil
import subprocess
import sys

# PyTorch builds: CUDA wheels carry their own CUDA runtime (only the NVIDIA
# driver is needed); the CPU index avoids downloading CUDA where it is useless.
TORCH_INDEX = {"nvidia": "https://download.pytorch.org/whl/cu128", "cpu": "https://download.pytorch.org/whl/cpu", "mps": None}
ENV_SIZE_GB = {"nvidia": 6.0, "cpu": 1.5, "mps": 1.5}


def data_dir():
    """The studio's own folder (its Python environment); delete it to uninstall."""
    if sys.platform == "win32":
        return os.path.join(os.environ.get("LOCALAPPDATA", os.path.expanduser(r"~\AppData\Local")), "DuneHD")
    if sys.platform == "darwin":
        return os.path.expanduser("~/Library/Application Support/DuneHD")
    return os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "dune-hd")


def env_python(env):
    return os.path.join(env, "Scripts", "python.exe") if sys.platform == "win32" else os.path.join(env, "bin", "python")


def nvidia_gpus():
    exe = shutil.which("nvidia-smi")
    if not exe:
        return []
    try:
        out = subprocess.run([exe, "--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"],
                             capture_output=True, text=True, timeout=15).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    gpus = []
    for line in out.strip().splitlines():
        parts = [x.strip() for x in line.split(",")]
        if len(parts) == 3 and parts[1].isdigit():
            gpus.append({"name": parts[0], "vram_gb": round(int(parts[1]) / 1024, 1), "driver": parts[2]})
    return gpus


def hardware():
    gpus = nvidia_gpus()
    if gpus:
        g = gpus[0]
        return {"kind": "nvidia", "label": f"NVIDIA {g['name']} ({g['vram_gb']:.0f} GB)", "gpus": gpus}
    if sys.platform == "darwin" and platform.machine() == "arm64":
        return {"kind": "mps", "label": "Apple GPU (MPS)"}
    return {"kind": "cpu", "label": "No supported GPU: CPU only (very slow)"}


CHECK = """
import json, sys
info = {"python": sys.version.split()[0]}
try:
    import torch
    info["torch"] = torch.__version__
    info["cuda"] = torch.cuda.is_available()
    info["cuda_device"] = torch.cuda.get_device_name(0) if info["cuda"] else None
    info["mps"] = bool(getattr(torch.backends, "mps", None) and torch.backends.mps.is_available())
    import diffusers, transformers, cv2, huggingface_hub
    info["diffusers"] = diffusers.__version__
    info["ok"] = True
except Exception as e:
    info["ok"] = False
    info["error"] = f"{type(e).__name__}: {e}"
print(json.dumps(info))
"""


def check_env(env):
    py = env_python(env)
    if not os.path.exists(py):
        return {"ok": False, "missing": True, "error": "not installed"}
    try:
        out = subprocess.run([py, "-c", CHECK], capture_output=True, text=True, timeout=180)
        return json.loads(out.stdout.strip().splitlines()[-1])
    except (OSError, subprocess.SubprocessError, ValueError, IndexError) as e:
        return {"ok": False, "error": str(e)}


def hf_hub_dir():
    if os.environ.get("HF_HUB_CACHE"):
        return os.environ["HF_HUB_CACHE"]
    return os.path.join(os.environ.get("HF_HOME", os.path.expanduser("~/.cache/huggingface")), "hub")


def model_status(models_json):
    """Each download of recipes/models.json: is its weights file in the cache?"""
    out = []
    for m in json.load(open(models_json))["downloads"]:
        snap = os.path.join(hf_hub_dir(), "models--" + m["repo"].replace("/", "--"), "snapshots")
        have = any(os.path.isfile(p) for p in glob.glob(os.path.join(snap, "*", "**", "*.safetensors"), recursive=True))
        out.append({**m, "cached": have})
    return out


def free_gb(path):
    while path and not os.path.exists(path):
        path = os.path.dirname(path)
    return round(shutil.disk_usage(path or os.sep).free / 2**30, 1)


def env_steps(env, kind, requirements):
    """Commands that create the generation environment for this machine."""
    py = env_python(env)
    uv = shutil.which("uv")
    steps = []
    if uv:
        steps.append(("create the Python environment", [uv, "venv", env, "--python", sys.executable]))
        pip = [uv, "pip", "install", "--python", py]
    else:
        steps.append(("create the Python environment", [sys.executable, "-m", "venv", env]))
        steps.append(("update pip", [py, "-m", "pip", "install", "--upgrade", "pip"]))
        pip = [py, "-m", "pip", "install"]
    torch = pip + ["torch"] + (["--index-url", TORCH_INDEX[kind]] if TORCH_INDEX.get(kind) else [])
    label = {"nvidia": "install PyTorch with CUDA (NVIDIA GPU)", "mps": "install PyTorch (Apple GPU)", "cpu": "install PyTorch (CPU)"}[kind]
    steps.append((label, torch))
    steps.append(("install the generation libraries", pip + ["-r", requirements]))
    steps.append(("check the environment", [py, "-c", "import torch, diffusers; print('torch', torch.__version__, 'cuda', torch.cuda.is_available())"]))
    return steps
