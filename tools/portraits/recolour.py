"""Colour-only variants of a generated face (e.g. Paul's eye stages), without generation.

    python recolour.py FACE_HD.png BASE_LOW.png OUT_DIR STAGE_LOW.png...

BASE_LOW is the low-res frame FACE_HD was generated from; each STAGE_LOW is
the same frame with some pixels recoloured by the game (another eye sprite).
The region recoloured by any stage (the eye) is found in the low-res frames
and, grown, in the HD face. The generated eye never sits exactly on the
pixel-art eye, so colours are matched by lightness rank, not position: an HD
eye pixel takes the colour change (CIELAB) of the low-res eye pixels of the
same rank (the white, the iris, the pupil), and keeps its own texture.
"""
import os
import sys

import cv2
import numpy as np
from PIL import Image

SIZE = 1024
BACKDROP = (24, 16, 12)  # as in proto.py
M = np.array([[0.4124, 0.3576, 0.1805], [0.2126, 0.7152, 0.0722], [0.0193, 0.1192, 0.9505]])
WHITE = np.array([0.9505, 1.0, 1.089])


def to_lab(rgb):
    c = rgb / 255.0
    c = np.where(c > 0.04045, ((c + 0.055) / 1.055) ** 2.4, c / 12.92)
    xyz = c @ M.T / WHITE
    f = np.where(xyz > 0.008856, np.cbrt(xyz), 7.787 * xyz + 16 / 116)
    return np.stack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]), 200 * (f[..., 1] - f[..., 2])], -1)


def to_rgb(lab):
    fy = (lab[..., 0] + 16) / 116
    f = np.stack([fy + lab[..., 1] / 500, fy, fy - lab[..., 2] / 200], -1)
    xyz = np.where(f > 0.2069, f ** 3, (f - 16 / 116) / 7.787) * WHITE
    c = xyz @ np.linalg.inv(M).T
    c = np.where(c > 0.0031308, 1.055 * np.clip(c, 0, None) ** (1 / 2.4) - 0.055, 12.92 * c)
    return np.clip(c * 255 + 0.5, 0, 255).astype(np.uint8)


def load_face(path):
    """The low-res frame padded to a square, as proto.py does (without torch)."""
    im = Image.open(path).convert("RGBA")
    side = max(im.size)
    sq = Image.new("RGBA", (side, side), BACKDROP + (255,))
    sq.alpha_composite(im, ((side - im.width) // 2, side - im.height))
    return sq.convert("RGB")


def lab(im):
    return to_lab(np.asarray(im, dtype=np.float64))


def eye_region(low, stages):
    """Low-res pixels any stage recolours."""
    a = np.asarray(low, dtype=np.int16)
    return np.any([np.abs(np.asarray(s, dtype=np.int16) - a).sum(-1) > 0 for s in stages], axis=0)


class Eye:
    """The HD eye: a feathered region around the low-res eye, minus skin, and each pixel's lightness rank."""

    def __init__(self, face, eye, grow=2, chroma=38.0, dark=30.0, feather=1.5):
        self.hd = lab(face)
        size = face.width
        k = np.ones((2 * grow + 1, 2 * grow + 1), np.uint8)
        near = cv2.resize(cv2.dilate(eye.astype(np.uint8), k), (size, size), interpolation=cv2.INTER_NEAREST) > 0
        # The opening (white, iris, pupil) is the least colourful part near the
        # eye; skin, lids and brows are orange-brown. Thin lash lines are opened away.
        cand = (np.hypot(self.hd[..., 1], self.hd[..., 2]) < chroma) & near
        ell = lambda r: cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (2 * r + 1, 2 * r + 1))
        m = cv2.morphologyEx(cand.astype(np.uint8), cv2.MORPH_CLOSE, ell(2))
        m = cv2.morphologyEx(m, cv2.MORPH_OPEN, ell(3))
        n, lab_, stats, _ = cv2.connectedComponentsWithStats(m)
        keep = np.zeros_like(m)
        for i in range(1, n):
            if stats[i, cv2.CC_STAT_AREA] >= 80:
                comp = (lab_ == i).astype(np.uint8)
                # Fill holes (catchlights, the pupil if it was too dark or bright).
                cs, _ = cv2.findContours(comp, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
                cv2.drawContours(keep, cs, -1, 1, -1)
        # Lashes: dark pixels on the opening's rim (the pupil is dark too, but enclosed).
        rim = keep > cv2.erode(keep, ell(4))
        keep[rim & (self.hd[..., 0] < dark)] = 0
        self.mask = keep > 0
        self.alpha = np.clip(cv2.GaussianBlur(keep.astype(np.float32), (0, 0), feather) * 1.3, 0, 1)
        inside = self.mask
        self.lsorted = np.sort(self.hd[inside][:, 0])
        self.rank = np.searchsorted(self.lsorted, self.hd[..., 0]) / max(len(self.lsorted) - 1, 1)


def recolour(eye, low, stage, region, blur=2.0, full=False):
    """The HD face with `stage`'s eye colours (an ndarray image); `region`: the low-res eye.

    `full`: take the stage's hue everywhere in the eye, not only where it
    changes (with `stage` = `low`: give the generated eye the frame's own colours)."""
    a, b = lab(low)[region], lab(stage)[region]
    order = np.argsort(a[:, 0], kind="stable")
    a, b = a[order], b[order]
    # Smooth along the rank so equal colours changed in some places only give a partial change.
    n = len(a)
    win = max(3, n // 12)
    kern = np.ones(win) / win
    rank_smooth = lambda v: np.stack([np.convolve(np.pad(v[:, i], win // 2, mode="edge"), kern, mode="valid")[:n] for i in range(3)], -1)
    a, b = rank_smooth(a), rank_smooth(b)
    idx = np.clip(eye.rank, 0, 1) * (n - 1)
    at = lambda v: np.stack([np.interp(idx, np.arange(n), v[:, i]) for i in range(3)], -1)
    src, dst = at(a), at(b)
    # Lightness: shifted (keeps the HD texture). Hue: replaced by the stage's,
    # as far as the stage changes it (adding a blue shift to a brown iris gives purple).
    amount = np.ones(idx.shape) if full else np.clip(np.linalg.norm(dst[..., 1:] - src[..., 1:], axis=-1) / 15, 0, 1)
    d = np.empty_like(eye.hd)
    d[..., 0] = dst[..., 0] - src[..., 0]
    d[..., 1:] = (dst[..., 1:] - eye.hd[..., 1:]) * amount[..., None]
    # Smooth the change inside the eye, so the rank lookup doesn't speckle.
    w = cv2.GaussianBlur(eye.mask.astype(np.float32), (0, 0), blur)
    for i in range(3):
        d[..., i] = cv2.GaussianBlur((d[..., i] * eye.mask).astype(np.float32), (0, 0), blur) / np.maximum(w, 1e-3)
    out = eye.hd + d * eye.alpha[..., None]
    out[..., 0] = np.clip(out[..., 0], 0, 100)
    return to_rgb(out)


def main():
    face_path, base_path, dst, *stage_paths = sys.argv[1:]
    os.makedirs(dst, exist_ok=True)
    face = Image.open(face_path).convert("RGB")
    assert face.width == SIZE
    low = load_face(base_path)
    stages = [load_face(p) for p in stage_paths]
    region = eye_region(low, stages)
    # The model picks its own iris colour: start from the frame's.
    face = Image.fromarray(recolour(Eye(face, region), low, low, region, full=True))
    eye = Eye(face, region)
    for path, st in zip(stage_paths, stages):
        Image.fromarray(recolour(eye, low, st, region)).save(os.path.join(dst, os.path.basename(path)))
        print(os.path.basename(path), flush=True)


if __name__ == "__main__":
    main()
