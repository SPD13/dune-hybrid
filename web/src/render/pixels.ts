// Palette-index frames to RGBA, for the Canvas2D paths.

const lut = new Uint32Array(256);

/** Palette-index frame → little-endian RGBA pixels. */
export function toRgba(f: Uint8Array, out: Uint32Array) {
  for (let i = 0; i < 256; i++) {
    const p = 64000 + i * 3;
    lut[i] = 0xff000000 | (f[p + 2] << 16) | (f[p + 1] << 8) | f[p];
  }
  for (let i = 0; i < 64000; i++) out[i] = lut[f[i]];
}
