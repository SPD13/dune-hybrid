// Mouse, pen, touch and keyboard → the emulated PC's mouse and keyboard.
//
// Touch has two modes:
//  - direct: the cursor jumps under the finger; touching presses the button.
//  - trackpad: dragging moves the cursor relatively; a quick tap clicks where
//    the cursor is; a two-finger tap is a right click. More precise on small
//    screens, where a finger hides what it touches.

import { SCANCODES } from "./keymap";
import type { TouchMode } from "./settings";

type Send = (msg: { type: "mouse"; x: number; y: number; buttons: number } | { type: "key"; code: number; pressed: boolean }) => void;

const TAP_MS = 250;
const TAP_SLOP_PX = 10;
const TRACKPAD_GAIN = 1.6;

export class Input {
  private x = 160;
  private y = 100;
  private buttons = 0;
  private touches = new Map<number, { x0: number; y0: number; lastX: number; lastY: number; t0: number; moved: boolean }>();
  private maxTouches = 0;

  constructor(
    private canvas: HTMLCanvasElement,
    private send: Send,
    private touchMode: () => TouchMode,
  ) {}

  attach() {
    const c = this.canvas;
    c.addEventListener("pointerdown", (e) => this.down(e));
    c.addEventListener("pointermove", (e) => this.move(e));
    c.addEventListener("pointerup", (e) => this.up(e));
    c.addEventListener("pointercancel", (e) => this.up(e));
    c.addEventListener("contextmenu", (e) => e.preventDefault());
    window.addEventListener("keydown", (e) => this.key(e, true));
    window.addEventListener("keyup", (e) => this.key(e, false));
  }

  /** Tap a key from an on-screen button. */
  tapKey(code: number) {
    this.send({ type: "key", code, pressed: true });
    setTimeout(() => this.send({ type: "key", code, pressed: false }), 100);
  }

  private toGame(e: PointerEvent) {
    const r = this.canvas.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(319, ((e.clientX - r.left) / r.width) * 320)),
      y: Math.max(0, Math.min(199, ((e.clientY - r.top) / r.height) * 200)),
    };
  }

  private emit() {
    this.send({ type: "mouse", x: Math.round(this.x), y: Math.round(this.y), buttons: this.buttons });
  }

  private down(e: PointerEvent) {
    e.preventDefault();
    this.canvas.focus();
    if (e.pointerType !== "touch") {
      Object.assign(this, this.toGame(e));
      this.buttons = e.buttons & 7;
      this.emit();
      return;
    }
    try {
      this.canvas.setPointerCapture(e.pointerId);
    } catch {
      // Synthetic or already-released pointers cannot be captured.
    }
    this.touches.set(e.pointerId, { x0: e.clientX, y0: e.clientY, lastX: e.clientX, lastY: e.clientY, t0: performance.now(), moved: false });
    this.maxTouches = Math.max(this.maxTouches, this.touches.size);
    if (this.touchMode() === "direct" && this.touches.size === 1) {
      Object.assign(this, this.toGame(e));
      this.buttons = 1;
      this.emit();
    }
  }

  private move(e: PointerEvent) {
    if (e.pointerType !== "touch") {
      Object.assign(this, this.toGame(e));
      this.buttons = e.buttons & 7;
      this.emit();
      return;
    }
    const t = this.touches.get(e.pointerId);
    if (!t) return;
    if (Math.hypot(e.clientX - t.x0, e.clientY - t.y0) > TAP_SLOP_PX) t.moved = true;
    if (this.touchMode() === "direct") {
      Object.assign(this, this.toGame(e));
    } else if (this.touches.size === 1) {
      const r = this.canvas.getBoundingClientRect();
      this.x = Math.max(0, Math.min(319, this.x + ((e.clientX - t.lastX) / r.width) * 320 * TRACKPAD_GAIN));
      this.y = Math.max(0, Math.min(199, this.y + ((e.clientY - t.lastY) / r.height) * 200 * TRACKPAD_GAIN));
    }
    t.lastX = e.clientX;
    t.lastY = e.clientY;
    this.emit();
  }

  private up(e: PointerEvent) {
    if (e.pointerType !== "touch") {
      this.buttons = e.buttons & 7;
      this.emit();
      return;
    }
    const t = this.touches.get(e.pointerId);
    this.touches.delete(e.pointerId);
    if (!t) return;
    if (this.touchMode() === "direct") {
      if (this.touches.size === 0) {
        this.buttons = 0;
        this.emit();
      }
    } else if (this.touches.size === 0) {
      const quick = performance.now() - t.t0 < TAP_MS && !t.moved;
      if (quick) this.click(this.maxTouches >= 2 ? 2 : 1);
    }
    if (this.touches.size === 0) this.maxTouches = 0;
  }

  private click(button: number) {
    this.buttons = button;
    this.emit();
    setTimeout(() => {
      this.buttons = 0;
      this.emit();
    }, 80);
  }

  private key(e: KeyboardEvent, pressed: boolean) {
    const code = SCANCODES[e.code];
    if (code === undefined) return;
    // Leave browser shortcuts alone (reload, devtools, …).
    if (e.metaKey || (e.ctrlKey && !["ControlLeft", "ControlRight"].includes(e.code))) return;
    e.preventDefault();
    if (pressed && e.repeat) return;
    this.send({ type: "key", code, pressed });
  }
}
