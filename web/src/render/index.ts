// The game picture on screen: a WebGL2 filter chain when available, else the
// plain Canvas2D path (CSS smoothing/scanlines only).

import type { Graphics } from "../settings";
import { Canvas2dRenderer } from "./canvas2d";
import { GlRenderer } from "./gl";
import { toRgba } from "./pixels";

export interface Renderer {
  readonly kind: "webgl2" | "canvas2d";
  /** A frame from the worker: 64000 palette indices then 256 RGB triples. */
  frame(f: Uint8Array): void;
  /** Changed rows of the HD screen (HD sprites and text), in order. */
  hd(rows: Uint8Array): void;
  /** Compare mode (WebGL2 only): default rendering right of `split` (0-1
   * of the width), the current one left of it; null turns it off. */
  setCompare(split: number | null): void;
  setGraphics(g: Graphics): void;
  /** The canvas's CSS size and the device pixel ratio to render at. */
  resize(cssWidth: number, cssHeight: number, dpr: number): void;
}

export function createRenderer(canvas: HTMLCanvasElement, stage: HTMLElement, touch: boolean): Renderer {
  const forced = import.meta.env.DEV && new URLSearchParams(location.search).get("renderer");
  return (forced !== "canvas2d" && GlRenderer.create(canvas, touch)) || new Canvas2dRenderer(canvas, stage);
}

/** A 160×120 JPEG of a frame, for save slots (independent of the filters). */
export function thumbnail(f: Uint8Array): string {
  const small = document.createElement("canvas");
  small.width = 320;
  small.height = 200;
  const ctx = small.getContext("2d")!;
  const img = ctx.createImageData(320, 200);
  toRgba(f, new Uint32Array(img.data.buffer));
  ctx.putImageData(img, 0, 0);
  const t = document.createElement("canvas");
  t.width = 160;
  t.height = 120;
  const tctx = t.getContext("2d")!;
  tctx.imageSmoothingQuality = "high";
  tctx.drawImage(small, 0, 0, 160, 120);
  return t.toDataURL("image/jpeg", 0.7);
}
