// Fallback renderer without WebGL2: the 320×200 canvas scaled by CSS, with
// the browser's smoothing or a CSS scanline overlay standing in for filters.

import type { Graphics } from "../settings";
import type { Renderer } from "./index";
import { toRgba } from "./pixels";

export class Canvas2dRenderer implements Renderer {
  readonly kind = "canvas2d";
  private ctx: CanvasRenderingContext2D;
  private image: ImageData;
  private rgba: Uint32Array;

  constructor(
    canvas: HTMLCanvasElement,
    private stage: HTMLElement,
  ) {
    canvas.width = 320;
    canvas.height = 200;
    this.ctx = canvas.getContext("2d", { alpha: false })!;
    this.image = this.ctx.createImageData(320, 200);
    this.rgba = new Uint32Array(this.image.data.buffer);
  }

  frame(f: Uint8Array) {
    toRgba(f, this.rgba);
    this.ctx.putImageData(this.image, 0, 0);
  }

  setGraphics(g: Graphics) {
    this.stage.dataset.display = g.crt !== "off" ? "crt" : g.output === "smooth" || g.scaler === "xbr" ? "smooth" : "sharp";
  }

  resize() {}

  hd() {}

  setCompare() {}
}
