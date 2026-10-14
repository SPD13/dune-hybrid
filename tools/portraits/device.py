"""Where diffusion runs: an NVIDIA GPU (CUDA) when there is one, Apple's GPU
(MPS) on a Mac, else the CPU; and how much memory saving the pipeline needs.

    import device
    dev = device.pick()            # {"name", "torch", "dtype", "vram_gb", "label"}
    device.configure(pipe, dev)    # moves the pipeline there, sized to its memory
"""
import os


def pick():
    import torch
    forced = os.environ.get("DUNE_HD_DEVICE")  # cuda, mps or cpu
    if (forced in (None, "cuda")) and torch.cuda.is_available():
        p = torch.cuda.get_device_properties(0)
        vram = p.total_memory / 2**30
        return {"name": "cuda", "torch": "cuda", "dtype": torch.float16, "vram_gb": round(vram, 1), "label": f"NVIDIA {p.name} ({vram:.0f} GB)"}
    if (forced in (None, "mps")) and torch.backends.mps.is_available():
        return {"name": "mps", "torch": "mps", "dtype": torch.float16, "vram_gb": None, "label": "Apple GPU (MPS)"}
    return {"name": "cpu", "torch": "cpu", "dtype": torch.float32, "vram_gb": None, "label": "CPU (very slow)"}


def configure(pipe, dev):
    """Put `pipe` on `dev`, saving memory only as much as it needs.

    NVIDIA: 12 GB or more holds SDXL + ControlNet at 1024 px as is (fastest);
    8-12 GB slices attention; under 8 GB the model is offloaded to the CPU part
    by part (slower, but fits 6 GB). Apple GPU: unified memory, slice
    attention (16 GB machines swap otherwise). The VAE always decodes in tiles.
    """
    vram = dev.get("vram_gb") or 0
    if dev["name"] == "cuda" and vram < 8:
        pipe.enable_model_cpu_offload()
    else:
        pipe.to(dev["torch"])
    if dev["name"] == "mps" or (dev["name"] == "cuda" and vram < 12):
        pipe.enable_attention_slicing()
    pipe.vae.enable_tiling()
    print(f"@device {dev['label']}", flush=True)
    return pipe
