// Dev only: records the project trailer (`?devtrailer`; `?devtrailer&stills`
// saves one still per second instead, to check the storyboard quickly, and
// `&only=<shot>` limits that to one shot, every half second; `?devtrailer&thumb`
// renders the video's thumbnail instead, `&at=<seconds>` to pick the moment).
//
// One scripted run of the game, stepped frame by frame in the worker, so
// the result never drops frames whatever the machine. Each frame goes
// through the app's own WebGL renderer (filters, HD pack, compare split),
// then titles and captions are drawn on a 1920×1080 canvas and the raw
// pixels are posted to the dev server, which pipes them into ffmpeg
// (vite.config.ts, devTrailer): out/trailer/trailer.mp4.
//
// Needs the game files and an HD pack imported in this browser.

import { packFile } from "../hdpack";
import { GlRenderer } from "../render/gl";
import { DEFAULTS, type Graphics, PRESETS, type Preset } from "../settings";
import { stored } from "../storage";
import type { FromWorker, ToWorker } from "../worker";

const W = 1920;
const H = 1080;
const FPS = 30;
const RATE = 48000;
/** The game picture: 4:3, centred. */
const GW = 1440;
const GX = (W - GW) / 2;

const SAND = "#e3b778";
const TEXT = "#f3e6d2";
const ACCENT = "#d9782b";
const SERIF = "Georgia, 'Times New Roman', serif";
const SANS = "system-ui, -apple-system, 'Segoe UI', sans-serif";

// ---------------------------------------------------------------- script

type Input = { t: number; msg: ToWorker };

/** Game input, in virtual seconds from boot. */
function script(): Input[] {
  const out: Input[] = [];
  let at = { x: 160, y: 100 };
  const mouse = (t: number, x: number, y: number, buttons = 0) => out.push({ t, msg: { type: "mouse", x, y, buttons } });
  const key = (t: number, code: number) => {
    out.push({ t, msg: { type: "key", code, pressed: true } });
    out.push({ t: t + 0.1, msg: { type: "key", code, pressed: false } });
  };
  /** Glide the pointer like a hand would, then rest. */
  const move = (t0: number, t1: number, x: number, y: number) => {
    const n = Math.max(1, Math.round((t1 - t0) * FPS));
    for (let i = 1; i <= n; i++) {
      const u = i / n;
      const e = u * u * (3 - 2 * u);
      mouse(t0 + (t1 - t0) * u, Math.round(at.x + (x - at.x) * e), Math.round(at.y + (y - at.y) * e));
    }
    at = { x, y };
  };
  const click = (t: number, x: number, y: number) => {
    move(t - 0.8, t - 0.1, x, y);
    mouse(t, x, y, 1);
    mouse(t + 0.15, x, y, 0);
  };
  key(61.5, 0x01); // Esc: from Irulan's prologue to the palace
  move(62.0, 62.2, 236, 118);
  click(76.0, 130, 179); // DUKE LETO ATREIDES
  click(79.0, 130, 163); // TALK TO ME
  click(115.0, 289, 177); // east: the balcony
  move(116.0, 116.8, 312, 150);
  return out.sort((a, b) => a.t - b.t);
}

// ---------------------------------------------------------------- shots

/** What one frame of a shot looks like (u: seconds into the shot). */
interface Look {
  graphics: Partial<Graphics>;
  compare?: number | null;
  /** Darken (0-1) and blur the game picture under titles. */
  dim?: number;
  blur?: number;
}

interface Shot {
  name: string;
  /** Virtual seconds of the game run shown. */
  from: number;
  to: number;
  look: (u: number) => Look;
  draw?: (ctx: CanvasRenderingContext2D, u: number, look: Look) => void;
  /** Fade from/to black (seconds). */
  fadeIn?: number;
  fadeOut?: number;
}

const preset = (p: Exclude<Preset, "custom">, hd: Graphics["hd"]): Partial<Graphics> => ({ preset: p, ...PRESETS[p], hd });
const HD = preset("xbr", "4");

const clamp = (v: number) => Math.min(1, Math.max(0, v));
const ramp = (u: number, a: number, b: number) => clamp((u - a) / (b - a));
const ease = (v: number) => v * v * (3 - 2 * v);
/** 0 → 1 over [a, a+f], 1 until b-f, → 0 at b. */
const span = (u: number, a: number, b: number, f = 0.5) => Math.min(ramp(u, a, a + f), 1 - ramp(u, b - f, b));

const SHOTS: Shot[] = [
  {
    name: "title",
    from: 0,
    to: 7.5,
    fadeIn: 0.6,
    fadeOut: 0.5,
    look: (u) => ({ graphics: HD, dim: 0.35 * (1 - ramp(u, 6, 7.5)) }),
    draw: (ctx, u) => {
      const a = span(u, 0.8, 7, 1.0);
      text(ctx, "DUNE", W / 2, 470, { font: `600 190px ${SERIF}`, color: SAND, alpha: a, spacing: 60, glow: 0.6 });
      text(ctx, "HYBRID", W / 2, 560, { font: `600 46px ${SERIF}`, color: ACCENT, alpha: ease(ramp(u, 1.6, 2.6)) * a, spacing: 26 });
      text(ctx, "Cryo's 1992 classic, reborn in your browser", W / 2, 650, { font: `400 34px ${SANS}`, color: TEXT, alpha: ease(ramp(u, 2.4, 3.4)) * a });
    },
  },
  {
    name: "prologue",
    from: 50.5,
    to: 61.0,
    fadeIn: 0.5,
    fadeOut: 0.4,
    look: () => ({ graphics: preset("original", "off") }),
    draw: (ctx, u) => caption(ctx, u, 1.0, 10.0, "The original CD game", "Voices, music and video, on our own 286 PC emulator in Rust and WebAssembly", true),
  },
  {
    name: "palace",
    from: 63.0,
    to: 86.0,
    fadeIn: 0.5,
    fadeOut: 0.4,
    look: (u) => {
      // The divider sweeps the HD pack in from the left, comes back to the
      // middle for a side by side look, then leaves the HD picture alone.
      let split = 0;
      if (u < 1.5) split = 0;
      else if (u < 4.5) split = ease(ramp(u, 1.5, 4.5));
      else if (u < 6.0) split = 1 - 0.5 * ease(ramp(u, 4.5, 6.0));
      else if (u < 9.5) split = 0.5;
      else split = 0.5 + 0.5 * ease(ramp(u, 9.5, 11.0));
      return { graphics: HD, compare: u < 11.0 ? split : null };
    },
    draw: (ctx, u, look) => {
      if (look.compare != null) compareLabels(ctx, look.compare, "HD ART PACK", "ORIGINAL", span(u, 1.2, 11.2, 0.4));
      caption(ctx, u, 1.5, 11.5, "HD art pack", "Sprites upscaled on your computer from your own copy of the game");
      caption(ctx, u, 13.5, 22.5, "Sharper characters", "Portraits and rooms in HD, with the game's palette effects intact");
    },
  },
  {
    name: "zoom",
    from: 106.0,
    to: 112.0,
    fadeIn: 0.4,
    fadeOut: 0.4,
    // Leto's close-up runs from about 3.2 to 4.6 s: split down the middle.
    look: (u) => ({ graphics: HD, compare: u < 4.7 ? 0.5 : u < 5.4 ? 0.5 + 0.5 * ease(ramp(u, 4.7, 5.4)) : null }),
    draw: (ctx, u, look) => {
      if (look.compare != null) compareLabels(ctx, look.compare, "HD", "ORIGINAL", span(u, 0, 5.4, 0.4));
      caption(ctx, u, 0.5, 5.8, "Smooth close-ups", "Zooms are drawn from the HD art, not blown-up pixels");
    },
  },
  {
    name: "filters",
    from: 117.0,
    to: 144.0,
    fadeIn: 0.5,
    fadeOut: 1.2,
    look: (u) => {
      const steps: [number, Partial<Graphics>][] = [
        [0, preset("original", "off")],
        [3.5, preset("pixel", "off")],
        [7.0, preset("xbr", "off")],
        [10.5, preset("crt", "off")],
        [14.0, HD],
      ];
      const graphics = steps.filter(([t]) => u >= t).pop()![1];
      const features = ramp(u, 14.0, 15.0);
      const end = ramp(u, 20.5, 21.5);
      return { graphics, dim: 0.55 * features + 0.25 * end, blur: 6 * features + 4 * end };
    },
    draw: (ctx, u) => {
      const names = ["ORIGINAL", "MMPX PIXEL ART", "xBR SMOOTHING", "CRT"];
      const i = Math.min(3, Math.floor(u / 3.5));
      if (u < 14.0) pill(ctx, names[i], GX + 40, 60, span(u - i * 3.5, 0, 3.5, 0.25), "left");
      caption(ctx, u, 0.4, 13.8, "Real-time filters", "Pixel-art upscalers, smoothing and CRT shaders, on desktop and phones");

      // What else is in the box.
      const lines = [
        "Plays the Spice Opera remastered soundtrack, if you own it",
        "Save anywhere, with instant snapshots",
        "Touch controls on phones and tablets",
        "Installs as an app and works offline",
        "Free and open source",
      ];
      const fa = span(u, 14.3, 20.6, 0.5);
      text(ctx, "And there's more", W / 2, 300, { font: `600 64px ${SERIF}`, color: SAND, alpha: fa });
      lines.forEach((l, k) => {
        const a = ease(ramp(u, 14.9 + 0.55 * k, 15.4 + 0.55 * k)) * fa;
        text(ctx, l, W / 2, 420 + 78 * k, { font: `400 38px ${SANS}`, color: TEXT, alpha: a });
      });

      // The end card.
      const ea = ease(ramp(u, 21.0, 22.0));
      text(ctx, "PLAY NOW", W / 2, 360, { font: `600 40px ${SANS}`, color: ACCENT, alpha: ea, spacing: 14 });
      text(ctx, "dune.spd13.us", W / 2, 490, { font: `600 120px ${SERIF}`, color: SAND, alpha: ea, glow: 0.5 });
      text(ctx, "Bring your own Dune CD files: the game is not included", W / 2, 600, { font: `400 34px ${SANS}`, color: TEXT, alpha: ease(ramp(u, 21.6, 22.4)) });
      text(ctx, "github.com/SPD13/dune-hybrid", W / 2, 670, { font: `400 34px ${SANS}`, color: TEXT, alpha: ease(ramp(u, 21.9, 22.7)) });
      text(ctx, "Unofficial fan project. Dune and its artwork belong to their respective owners.", W / 2, 990, {
        font: `400 22px ${SANS}`,
        color: "#b9a68c",
        alpha: ease(ramp(u, 22.2, 23.0)),
      });
    },
  },
];

// ---------------------------------------------------------------- drawing

interface TextStyle {
  font: string;
  color: string;
  alpha: number;
  spacing?: number;
  glow?: number;
  align?: CanvasTextAlign;
}

function text(ctx: CanvasRenderingContext2D, s: string, x: number, y: number, st: TextStyle) {
  if (st.alpha <= 0) return;
  ctx.save();
  ctx.globalAlpha = st.alpha;
  ctx.font = st.font;
  ctx.fillStyle = st.color;
  ctx.textAlign = st.align ?? "center";
  ctx.textBaseline = "alphabetic";
  ctx.letterSpacing = `${st.spacing ?? 0}px`;
  // Spacing pads the right of every letter, the last one included.
  const dx = st.align === "left" ? 0 : (st.spacing ?? 0) / 2;
  ctx.shadowColor = "rgba(0,0,0,0.85)";
  ctx.shadowBlur = 18;
  ctx.shadowOffsetY = 3;
  ctx.fillText(s, x + dx, y);
  if (st.glow) {
    ctx.shadowColor = `rgba(217,120,43,${st.glow})`;
    ctx.shadowBlur = 40;
    ctx.shadowOffsetY = 0;
    ctx.fillText(s, x + dx, y);
  }
  ctx.restore();
}

/** A lower-third caption shown from a to b (shot seconds). */
/** A caption shown from a to b (shot seconds), at the bottom or, where the
 * game shows its own subtitles there, at the top. */
function caption(ctx: CanvasRenderingContext2D, u: number, a: number, b: number, title: string, sub: string, top = false) {
  const al = span(u, a, b, 0.45);
  if (al <= 0) return;
  ctx.save();
  ctx.globalAlpha = al;
  const g = top ? ctx.createLinearGradient(0, 300, 0, 0) : ctx.createLinearGradient(0, H - 300, 0, H);
  g.addColorStop(0, "rgba(10,6,3,0)");
  g.addColorStop(0.55, "rgba(10,6,3,0.72)");
  g.addColorStop(1, "rgba(10,6,3,0.88)");
  ctx.fillStyle = g;
  ctx.fillRect(0, top ? 0 : H - 300, W, 300);
  ctx.restore();
  const rise = 14 * (1 - ease(ramp(u, a, a + 0.6)));
  const y = top ? 120 : H - 128;
  text(ctx, title, W / 2, y + rise, { font: `600 60px ${SERIF}`, color: SAND, alpha: al });
  text(ctx, sub, W / 2, y + 58 + rise, { font: `400 32px ${SANS}`, color: TEXT, alpha: al * ease(ramp(u, a + 0.2, a + 0.8)) });
}

function pill(ctx: CanvasRenderingContext2D, s: string, x: number, y: number, alpha: number, align: "left" | "right") {
  if (alpha <= 0) return;
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.font = `600 26px ${SANS}`;
  ctx.letterSpacing = "4px";
  const w = ctx.measureText(s).width + 40;
  const left = align === "left" ? x : x - w;
  ctx.fillStyle = "rgba(18,12,7,0.78)";
  ctx.strokeStyle = SAND;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.roundRect(left, y, w, 50, 25);
  ctx.fill();
  ctx.stroke();
  ctx.fillStyle = SAND;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillText(s, left + 22, y + 26);
  ctx.restore();
}

/** The compare divider, as the app draws it, with a label on each side. */
function compareLabels(ctx: CanvasRenderingContext2D, split: number, left: string, right: string, alpha: number) {
  const x = GX + split * GW;
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.shadowColor = "rgba(0,0,0,0.6)";
  ctx.shadowBlur = 10;
  ctx.fillStyle = SAND;
  ctx.fillRect(x - 2, 0, 4, H);
  ctx.beginPath();
  ctx.arc(x, H / 2, 30, 0, Math.PI * 2);
  ctx.fillStyle = "rgba(18,12,7,0.85)";
  ctx.fill();
  ctx.lineWidth = 3;
  ctx.strokeStyle = SAND;
  ctx.stroke();
  ctx.fillStyle = SAND;
  for (const d of [-1, 1]) {
    ctx.beginPath();
    ctx.moveTo(x + d * 20, H / 2);
    ctx.lineTo(x + d * 8, H / 2 - 10);
    ctx.lineTo(x + d * 8, H / 2 + 10);
    ctx.fill();
  }
  ctx.restore();
  if (split > 0.2) pill(ctx, left, GX + 40, 60, alpha * ramp(split, 0.2, 0.3), "left");
  if (split < 0.8) pill(ctx, right, GX + GW - 40, 60, alpha * (1 - ramp(split, 0.7, 0.8)), "right");
}

// ---------------------------------------------------------------- thumbnail

/** Framing: the game picture at `scale`× with its top at `y`, the divider
 * at `split` of the game's width, landing at canvas x `divider`. */
interface Frame {
  scale: number;
  split: number;
  divider: number;
  y: number;
}

function drawThumbnail(ctx: CanvasRenderingContext2D, gl: HTMLCanvasElement, f: Frame) {
  ctx.fillStyle = "#000";
  ctx.fillRect(0, 0, W, H);
  ctx.drawImage(gl, f.divider - f.split * 320 * f.scale, f.y);
  const divider = f.divider;
  // Fade out under the scene (rows 0-152): the control panel is not the point.
  const bottom = f.y + 152 * f.scale;
  if (bottom < H) {
    const b = ctx.createLinearGradient(0, bottom - 220, 0, bottom);
    b.addColorStop(0, "rgba(0,0,0,0)");
    b.addColorStop(1, "rgba(0,0,0,1)");
    ctx.fillStyle = b;
    ctx.fillRect(0, bottom - 220, W, 220);
    ctx.fillStyle = "#000";
    ctx.fillRect(0, bottom, W, H - bottom);
  }

  // Darken the right for the title, and the edges a little.
  const g = ctx.createLinearGradient(1200, 0, W, 0);
  g.addColorStop(0, "rgba(8,5,2,0)");
  g.addColorStop(0.35, "rgba(8,5,2,0.78)");
  g.addColorStop(1, "rgba(8,5,2,0.92)");
  ctx.fillStyle = g;
  ctx.fillRect(1200, 0, W - 1200, H);
  const v = ctx.createRadialGradient(W * 0.4, H / 2, H * 0.45, W * 0.4, H / 2, H * 1.05);
  v.addColorStop(0, "rgba(0,0,0,0)");
  v.addColorStop(1, "rgba(0,0,0,0.6)");
  ctx.fillStyle = v;
  ctx.fillRect(0, 0, W, H);

  // The divider, bolder than in the video.
  ctx.save();
  ctx.shadowColor = "rgba(0,0,0,0.7)";
  ctx.shadowBlur = 16;
  ctx.fillStyle = SAND;
  ctx.fillRect(divider - 4, 0, 8, Math.min(H, bottom));
  ctx.beginPath();
  ctx.arc(divider, H * 0.62, 46, 0, Math.PI * 2);
  ctx.fillStyle = "rgba(18,12,7,0.9)";
  ctx.fill();
  ctx.lineWidth = 6;
  ctx.strokeStyle = SAND;
  ctx.stroke();
  ctx.fillStyle = SAND;
  for (const d of [-1, 1]) {
    ctx.beginPath();
    ctx.moveTo(divider + d * 30, H * 0.62);
    ctx.lineTo(divider + d * 11, H * 0.62 - 16);
    ctx.lineTo(divider + d * 11, H * 0.62 + 16);
    ctx.fill();
  }
  ctx.restore();
  bigPill(ctx, "HD", divider - 40, 70, "right", ACCENT, "#1a0f05");
  bigPill(ctx, "1992", divider + 40, 70, "left", "rgba(18,12,7,0.85)", SAND);

  // The title.
  const cx = 1590;
  text(ctx, "DUNE", cx, 450, { font: `700 150px ${SERIF}`, color: SAND, alpha: 1, spacing: 18, glow: 0.9 });
  text(ctx, "REBORN IN HD", cx, 565, { font: `800 66px ${SANS}`, color: TEXT, alpha: 1, spacing: 2 });
  ctx.save();
  ctx.fillStyle = ACCENT;
  ctx.fillRect(cx - 150, 612, 300, 6);
  ctx.restore();
  text(ctx, "Play it in your browser", cx, 690, { font: `500 46px ${SANS}`, color: SAND, alpha: 1 });
}

function bigPill(ctx: CanvasRenderingContext2D, s: string, x: number, y: number, align: "left" | "right", fill: string, ink: string) {
  ctx.save();
  ctx.font = `800 64px ${SANS}`;
  ctx.letterSpacing = "6px";
  const w = ctx.measureText(s).width + 64;
  const left = align === "left" ? x : x - w;
  ctx.shadowColor = "rgba(0,0,0,0.7)";
  ctx.shadowBlur = 20;
  ctx.fillStyle = fill;
  ctx.beginPath();
  ctx.roundRect(left, y, w, 100, 50);
  ctx.fill();
  ctx.shadowColor = "transparent";
  ctx.lineWidth = 4;
  ctx.strokeStyle = SAND;
  ctx.stroke();
  ctx.fillStyle = ink;
  ctx.textBaseline = "middle";
  ctx.fillText(s, left + 32, y + 54);
  ctx.restore();
}

// ---------------------------------------------------------------- run

export async function recordTrailer() {
  const params = new URLSearchParams(location.search);
  const stills = params.has("stills");
  /** Stills of one shot only, every half second. */
  const only = params.get("only");
  const status = document.createElement("div");
  status.style.cssText = "position:fixed;left:0;right:0;bottom:0;z-index:61;padding:6px 12px;background:#000;color:#e3b778;font:14px system-ui";
  const out = document.createElement("canvas");
  out.width = W;
  out.height = H;
  out.style.cssText = "position:fixed;inset:0;z-index:60;width:100vw;height:calc(100vw * 9 / 16);background:#000";
  document.body.append(out, status);
  const say = (s: string) => {
    status.textContent = s;
    console.log(`[trailer] ${s}`);
  };
  const ctx = out.getContext("2d", { willReadFrequently: true })!;

  const gl = document.createElement("canvas");
  const renderer = GlRenderer.create(gl, false);
  if (!renderer) throw new Error("no WebGL2");
  renderer.resize(GW, H, 1);

  // The files imported in this browser (`?devfiles`, `?devpack`): Chrome
  // cannot hold a 400 MB download as a Blob, but reads OPFS files lazily.
  say("loading game files and HD pack…");
  const [exeFile, dat, pack] = await Promise.all([stored("DNCDPRG.EXE"), stored("DUNE.DAT"), packFile()]);
  if (!exeFile || !dat || !pack) throw new Error("import the game files and an HD pack first (?devfiles&devpack)");
  const exe = await exeFile.arrayBuffer();

  const worker = new Worker(new URL("../worker.ts", import.meta.url), { type: "module" });
  let wake: ((m: FromWorker) => void) | null = null;
  const started = new Promise<void>((done) => {
    worker.onmessage = (e: MessageEvent<FromWorker>) => {
      const m = e.data;
      if (m.type === "started") done();
      else if (m.type === "stepped") wake?.(m);
      else if (m.type === "log") console.log("[emu]", m.text);
    };
  });
  const send = (m: ToWorker) => worker.postMessage(m);
  send({
    type: "start",
    exe,
    dat,
    hdPack: pack,
    cmdline: "ADP330 SBP2227 ENG",
    saves: [],
    music: DEFAULTS.music,
    voices: DEFAULTS.voices,
    batterySaver: true,
    replacedSongs: 0,
    hd: 4,
    hdVisible: true,
    manual: true,
  });
  await started;

  // Virtual time, stepped exactly; input goes in between steps.
  const inputs = script();
  let now = 0;
  let next = 0;
  type Stepped = Extract<FromWorker, { type: "stepped" }>;
  let last = null as Stepped | null;
  const stepTo = async (t: number) => {
    while (next < inputs.length && inputs[next].t <= t) {
      const target = inputs[next].t;
      if (target > now) await step(target);
      send(inputs[next++].msg);
    }
    if (t > now) await step(t);
  };
  const audio: Float32Array[] = [];
  let keepAudio = false;
  const step = (t: number) =>
    new Promise<void>((done) => {
      wake = (m) => {
        last = m as Stepped;
        now = t;
        if (last!.hd.length > 8) renderer.hd(last!.hd);
        if (keepAudio) audio.push(last!.audio);
        done();
      };
      send({ type: "step", until: t * 1000 });
    });

  if (params.has("thumb")) {
    const at = Number(params.get("at") ?? 109.8);
    say(`thumbnail: running to ${at}s`);
    for (let t = now; t < at; ) {
      t = Math.min(at, t + 0.5);
      await stepTo(t);
    }
    const num = (k: string, d: number) => Number(params.get(k) ?? d);
    const frame = { scale: num("scale", 6.8), split: num("split", 0.578), divider: num("divider", 958), y: num("y", 0) };
    renderer.resize(frame.scale * 320, frame.scale * 200, 1);
    renderer.setGraphics({ ...DEFAULTS.graphics, ...HD });
    renderer.setCompare(frame.split);
    renderer.frame(last!.frame);
    drawThumbnail(ctx, gl, frame);
    const png = await new Promise<Blob>((done) => out.toBlob((b) => done(b!), "image/png"));
    await fetch(`dev-trailer/still?name=${params.get("name") ?? "thumbnail"}`, { method: "POST", body: png });
    say("done: out/trailer/thumbnail.png");
    return;
  }

  const post = (path: string, body: BodyInit) => fetch(`dev-trailer/${path}`, { method: "POST", body }).then((r) => r.json());
  if (!stills) await post(`start?w=${W}&h=${H}&fps=${FPS}`, "");

  const total = SHOTS.reduce((n, s) => n + Math.round((s.to - s.from) * FPS), 0);
  let written = 0;
  const t0 = performance.now();
  let lastGraphics = "";
  let lastCompare: number | null | undefined;
  for (const shot of SHOTS.filter((s) => !only || s.name === only)) {
    say(`${shot.name}: running to ${shot.from}s`);
    // Skip ahead in large steps (input still lands on time).
    for (let t = now; t < shot.from; ) {
      t = Math.min(shot.from, t + 0.5);
      await stepTo(t);
    }
    const frames = Math.round((shot.to - shot.from) * FPS);
    keepAudio = true;
    audio.length = 0;
    for (let i = 0; i < frames; i++) {
      await stepTo(shot.from + (i + 1) / FPS);
      const u = i / FPS;
      const look = shot.look(u);
      const g = { ...DEFAULTS.graphics, ...look.graphics };
      if (JSON.stringify(g) !== lastGraphics) {
        lastGraphics = JSON.stringify(g);
        renderer.setGraphics(g);
      }
      const split = look.compare ?? null;
      if (split !== lastCompare) {
        lastCompare = split;
        renderer.setCompare(split);
      }
      renderer.frame(last!.frame);

      ctx.fillStyle = "#000";
      ctx.fillRect(0, 0, W, H);
      ctx.save();
      ctx.filter = look.blur || look.dim ? `blur(${look.blur ?? 0}px) brightness(${1 - (look.dim ?? 0)})` : "none";
      ctx.drawImage(gl, GX, 0, GW, H);
      ctx.restore();
      shot.draw?.(ctx, u, look);
      const fade = Math.min(shot.fadeIn ? ramp(u, 0, shot.fadeIn) : 1, shot.fadeOut ? 1 - ramp(u, shot.to - shot.from - shot.fadeOut, shot.to - shot.from) : 1);
      if (fade < 1) {
        ctx.fillStyle = `rgba(0,0,0,${1 - fade})`;
        ctx.fillRect(0, 0, W, H);
      }

      if (stills) {
        const every = only ? FPS / 2 : FPS;
        if (i % every === Math.floor(every / 2)) {
          const png = await new Promise<Blob>((done) => out.toBlob((b) => done(b!), "image/png"));
          await post(`still?name=${shot.name}-${String(Math.round(u * 10)).padStart(3, "0")}`, png);
        }
      } else {
        await post("frame", ctx.getImageData(0, 0, W, H).data);
      }
      written++;
      if (written % 15 === 0) {
        const rate = written / ((performance.now() - t0) / 1000);
        say(`${shot.name}: frame ${written}/${total} (${rate.toFixed(1)} fps, ${((total - written) / rate).toFixed(0)} s left)`);
      }
    }
    keepAudio = false;
    if (!stills) await post("audio", new Blob([shotAudio(audio, frames, shot) as Float32Array<ArrayBuffer>]));
  }
  if (stills) {
    say(`done: ${written} frames checked, stills in out/trailer`);
    return;
  }
  say("encoding…");
  const r = await post(`finish?seconds=${total / FPS}`, "");
  say(r.ok ? `done: ${r.path}` : "ffmpeg failed (see the dev server log)");
}

/** A shot's sound: exactly its length in samples, faded at the cuts. */
function shotAudio(parts: Float32Array[], frames: number, shot: Shot): Float32Array {
  const n = Math.round((frames / FPS) * RATE) * 2;
  const pcm = new Float32Array(n);
  let o = 0;
  for (const p of parts) {
    pcm.set(p.subarray(0, Math.max(0, n - o)), o);
    o += p.length;
    if (o >= n) break;
  }
  const fade = (sec: number | undefined, from: "start" | "end") => {
    const len = Math.round((sec ?? 0.05) * RATE);
    for (let i = 0; i < len; i++) {
      const g = i / len;
      const at = from === "start" ? i : n / 2 - 1 - i;
      pcm[at * 2] *= g;
      pcm[at * 2 + 1] *= g;
    }
  };
  fade(shot.fadeIn, "start");
  fade(shot.fadeOut, "end");
  return pcm;
}
