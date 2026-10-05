// WebGL2 filter chain for the game picture:
//
//   indices (R8UI 320×200) + palette (256×1)
//     → palette pass → RGB 320×200
//     → optional upscaler (MMPX 2×, Scale4x, xBR-lv2 4×)
//     → with HD sprites and text: the worker's k× HD screen resolved with
//       the palette, falling back to the picture above where needed
//     → output pass to the canvas: nearest / sharp-bilinear / smooth,
//       plus light scanlines; or the crt-lottes pass on the 320×200 RGB.
//
// Intermediate targets keep image rows top-down; the last pass flips.

import type { Graphics } from "../settings";
import crtLottesFrag from "./shaders/crt-lottes.frag?raw";
import hdFrag from "./shaders/hd.frag?raw";
import vert from "./shaders/fullscreen.vert?raw";
import mmpxFrag from "./shaders/mmpx.frag?raw";
import outputFrag from "./shaders/output.frag?raw";
import paletteFrag from "./shaders/palette.frag?raw";
import scale2xFrag from "./shaders/scale2x.frag?raw";
import xbrFrag from "./shaders/xbr-lv2.frag?raw";
import type { Renderer } from "./index";

const W = 320;
const H = 200;
const OUTPUT_MODE = { nearest: 0, sharp: 1, smooth: 2 } as const;

interface Program {
  prog: WebGLProgram;
  u: (name: string) => WebGLUniformLocation | null;
}

interface Target {
  tex: WebGLTexture;
  fb: WebGLFramebuffer;
  w: number;
  h: number;
}

export class GlRenderer implements Renderer {
  readonly kind = "webgl2";
  private gl: WebGL2RenderingContext;
  private programs!: Record<"palette" | "scale2x" | "mmpx" | "xbr" | "output" | "lottes" | "hd", Program>;
  /** The HD screen texels (k×), when the worker sends them. */
  private hdTex: WebGLTexture | null = null;
  private hdScale = 0;
  private hdReady = false;
  /** Debug: tint HD pixels green and fallbacks red (`?hdoverlay`). */
  hdOverlay = import.meta.env.DEV && new URLSearchParams(location.search).has("hdoverlay");
  private index!: WebGLTexture;
  private palette!: WebGLTexture;
  private targets = new Map<string, Target>();
  private vao!: WebGLVertexArrayObject;
  private frameData: Uint8Array | null = null;
  private graphics: Graphics | null = null;
  private lost = false;
  private queued = 0;
  private bloom: boolean;

  /** Time of the last draw call sequence (CPU side), for diagnostics. */
  lastDrawMs = 0;

  static create(canvas: HTMLCanvasElement, touch: boolean): GlRenderer | null {
    const gl = canvas.getContext("webgl2", { alpha: false, antialias: false, depth: false, stencil: false, preserveDrawingBuffer: false, powerPreference: "default" });
    if (!gl) return null;
    try {
      return new GlRenderer(canvas, gl, touch);
    } catch (err) {
      console.warn("WebGL2 renderer unavailable", err);
      return null;
    }
  }

  private constructor(
    private canvas: HTMLCanvasElement,
    gl: WebGL2RenderingContext,
    touch: boolean,
  ) {
    this.gl = gl;
    this.bloom = !touch;
    this.init();
    canvas.addEventListener("webglcontextlost", (e) => {
      e.preventDefault();
      this.lost = true;
    });
    canvas.addEventListener("webglcontextrestored", () => {
      this.targets.clear();
      this.init();
      this.lost = false;
      this.redraw();
    });
  }

  private init() {
    const gl = this.gl;
    const compile = (frag: string): Program => {
      const shader = (type: number, src: string) => {
        const s = gl.createShader(type)!;
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS) && !gl.isContextLost()) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
        return s;
      };
      const prog = gl.createProgram()!;
      gl.attachShader(prog, shader(gl.VERTEX_SHADER, vert));
      gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, frag));
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS) && !gl.isContextLost()) throw new Error(gl.getProgramInfoLog(prog) ?? "link");
      const cache = new Map<string, WebGLUniformLocation | null>();
      const u = (name: string) => {
        if (!cache.has(name)) cache.set(name, gl.getUniformLocation(prog, name));
        return cache.get(name)!;
      };
      return { prog, u };
    };
    this.programs = {
      palette: compile(paletteFrag),
      scale2x: compile(scale2xFrag),
      mmpx: compile(mmpxFrag),
      xbr: compile(xbrFrag),
      output: compile(outputFrag),
      lottes: compile(crtLottesFrag),
      hd: compile(hdFrag),
    };
    this.vao = gl.createVertexArray()!;

    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    this.index = this.texture(gl.R8UI, W, H, gl.NEAREST);
    this.palette = this.texture(gl.RGB8, 256, 1, gl.NEAREST);
  }

  private texture(format: number, w: number, h: number, filter: number) {
    const gl = this.gl;
    const tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, format, w, h);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return tex;
  }

  private target(name: string, w: number, h: number): Target {
    let t = this.targets.get(name);
    if (t && t.w === w && t.h === h) return t;
    const gl = this.gl;
    if (t) {
      gl.deleteTexture(t.tex);
      gl.deleteFramebuffer(t.fb);
    }
    const tex = this.texture(gl.RGBA8, w, h, gl.LINEAR);
    const fb = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    t = { tex, fb, w, h };
    this.targets.set(name, t);
    return t;
  }

  private pass(p: Program, out: Target | null, inputs: WebGLTexture[], uniforms: (u: Program["u"]) => void) {
    const gl = this.gl;
    gl.bindFramebuffer(gl.FRAMEBUFFER, out ? out.fb : null);
    gl.viewport(0, 0, out ? out.w : this.canvas.width, out ? out.h : this.canvas.height);
    gl.useProgram(p.prog);
    inputs.forEach((tex, i) => {
      gl.activeTexture(gl.TEXTURE0 + i);
      gl.bindTexture(gl.TEXTURE_2D, tex);
    });
    uniforms(p.u);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  frame(f: Uint8Array) {
    this.frameData = f;
    if (this.lost) return;
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.index);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, W, H, gl.RED_INTEGER, gl.UNSIGNED_BYTE, f, 0);
    gl.bindTexture(gl.TEXTURE_2D, this.palette);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, 256, 1, gl.RGB, gl.UNSIGNED_BYTE, f, W * H);
    this.draw();
  }

  /** Rows of the HD screen (header: k, 0, first row u16, end row u16,
   * fallback count u16). Uploaded as they arrive: they are incremental. */
  hd(hd: Uint8Array) {
    if (this.lost) return;
    const gl = this.gl;
    const k = hd[0];
    const y0 = hd[2] | (hd[3] << 8);
    const y1 = hd[4] | (hd[5] << 8);
    if (k !== this.hdScale || !this.hdTex) {
      this.hdReady = false;
      if (this.hdTex) gl.deleteTexture(this.hdTex);
      this.hdTex = this.texture(gl.RGBA8, W * k, H * k, gl.NEAREST);
      this.hdScale = k;
    }
    // The first message after (re)enabling covers the whole screen.
    if (y0 === 0 && y1 === H * k) this.hdReady = true;
    if (y1 > y0) {
      gl.bindTexture(gl.TEXTURE_2D, this.hdTex);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, y0, W * k, y1 - y0, gl.RGBA, gl.UNSIGNED_BYTE, hd, 8);
    }
  }

  setGraphics(g: Graphics) {
    this.graphics = { ...g };
    this.redraw();
  }

  resize(cssWidth: number, cssHeight: number, dpr: number) {
    const w = Math.max(1, Math.round(cssWidth * dpr));
    const h = Math.max(1, Math.round(cssHeight * dpr));
    if (w === this.canvas.width && h === this.canvas.height) return;
    this.canvas.width = w;
    this.canvas.height = h;
    this.redraw();
  }

  /** Redraw the last frame on the next animation frame (settings, size). */
  /** Dev: average milliseconds per draw for each setting, GPU included. */
  bench(settings: Graphics[], n = 60): number[] {
    const px = new Uint8Array(4);
    const keep = this.graphics;
    const out = settings.map((g) => {
      this.graphics = g;
      this.draw();
      this.gl.readPixels(0, 0, 1, 1, this.gl.RGBA, this.gl.UNSIGNED_BYTE, px);
      const t0 = performance.now();
      for (let i = 0; i < n; i++) this.draw();
      this.gl.readPixels(0, 0, 1, 1, this.gl.RGBA, this.gl.UNSIGNED_BYTE, px);
      return (performance.now() - t0) / n;
    });
    this.graphics = keep;
    this.draw();
    return out;
  }

  private redraw() {
    if (this.queued) return;
    this.queued = requestAnimationFrame(() => {
      this.queued = 0;
      this.draw();
    });
  }

  private draw() {
    const g = this.graphics;
    if (!this.frameData || !g || this.lost) return;
    const t0 = performance.now();
    const gl = this.gl;
    const P = this.programs;
    gl.bindVertexArray(this.vao);

    const base = this.target("base", W, H);
    this.pass(P.palette, base, [this.index, this.palette], (u) => {
      gl.uniform1i(u("uIndex"), 0);
      gl.uniform1i(u("uPal"), 1);
    });

    if (g.crt === "lottes") {
      this.pass(P.lottes, null, [base.tex], (u) => {
        gl.uniform1i(u("uSrc"), 0);
        gl.uniform2f(u("uSrcSize"), W, H);
        gl.uniform1f(u("uStrength"), g.crtStrength);
        gl.uniform1i(u("uBloom"), this.bloom ? 1 : 0);
      });
    } else {
      let src = this.upscale(g, base);
      if (g.hd !== "off" && this.hdTex && this.hdScale && this.hdReady) {
        const k = this.hdScale;
        const low = src;
        const out = this.target("hd", W * k, H * k);
        this.pass(P.hd, out, [this.hdTex, this.palette, low.tex], (u) => {
          gl.uniform1i(u("uHd"), 0);
          gl.uniform1i(u("uPal"), 1);
          gl.uniform1i(u("uLow"), 2);
          gl.uniform2i(u("uLowSize"), low.w, low.h);
          gl.uniform2f(u("uHdSize"), W * k, H * k);
          gl.uniform1i(u("uOverlay"), this.hdOverlay ? 1 : 0);
        });
        src = out;
      }
      this.pass(P.output, null, [src.tex], (u) => {
        gl.uniform1i(u("uSrc"), 0);
        gl.uniform2f(u("uSrcSize"), src.w, src.h);
        gl.uniform2f(u("uOutSize"), this.canvas.width, this.canvas.height);
        gl.uniform1i(u("uMode"), OUTPUT_MODE[g.output]);
        gl.uniform1f(u("uScan"), g.crt === "scanlines" ? g.crtStrength : 0);
      });
    }
    this.lastDrawMs = performance.now() - t0;
  }

  private upscale(g: Graphics, base: Target): Target {
    const gl = this.gl;
    const P = this.programs;
    const double = (p: Program, src: Target, name: string) => {
      const out = this.target(name, src.w * 2, src.h * 2);
      this.pass(p, out, [src.tex], (u) => {
        gl.uniform1i(u("uSrc"), 0);
        gl.uniform2i(u("uSrcSize"), src.w, src.h);
      });
      return out;
    };
    switch (g.scaler) {
      case "mmpx":
        // One pass: a second one erodes the 1-pixel strokes of the game's font.
        return double(P.mmpx, base, "x2");
      case "epx":
        return double(P.scale2x, double(P.scale2x, base, "x2"), "x4");
      case "xbr": {
        const out = this.target("x4", W * 4, H * 4);
        this.pass(P.xbr, out, [base.tex], (u) => {
          gl.uniform1i(u("uSrc"), 0);
          gl.uniform2i(u("uSrcSize"), W, H);
          gl.uniform1f(u("uScale"), 4);
          gl.uniform1f(u("uAmount"), g.xbrAmount);
        });
        return out;
      }
      default:
        return base;
    }
  }
}
