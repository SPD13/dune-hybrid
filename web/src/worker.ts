// Runs the emulated PC. The emulator never blocks, so a timer loop drives it:
// each tick advances virtual time by the wall time elapsed (times the speed).
import init, { Emu } from "./pkg/web.js";

export type ToWorker =
  | {
      type: "start";
      exe: ArrayBuffer;
      dat: Blob;
      cmdline: string;
      saves: { name: string; data: Uint8Array }[];
      snapshot?: Uint8Array;
      music: number;
      voices: number;
      batterySaver: boolean;
    }
  | { type: "key"; code: number; pressed: boolean }
  | { type: "mouse"; x: number; y: number; buttons: number }
  | { type: "pause"; paused: boolean }
  | { type: "speed"; factor: number }
  | { type: "volume"; music: number; voices: number }
  | { type: "batterySaver"; on: boolean }
  | { type: "snapshot"; id: number }
  | { type: "load"; data: Uint8Array };

export type FromWorker =
  | { type: "started" }
  | { type: "frame"; frame: Uint8Array }
  | { type: "audio"; audio: Float32Array }
  | { type: "save"; name: string; data: Uint8Array }
  | { type: "snapshot"; id: number; data: Uint8Array }
  | { type: "loaded"; ok: boolean; message?: string }
  | { type: "stats"; virtual: number; wall: number; busy: number }
  | { type: "log"; text: string }
  | { type: "exit" }
  | { type: "error"; message: string };

let emu: Emu | null = null;
let paused = false;
let speed = 1;
let timer: ReturnType<typeof setTimeout> | undefined;
const post = (msg: FromWorker, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(msg, transfer);

function loop() {
  let last = performance.now();
  const t0 = last;
  let lastStats = last;
  let busy = 0;
  const tick = () => {
    const now = performance.now();
    const elapsed = Math.min(now - last, 100);
    last = now;
    if (paused || !emu) {
      timer = setTimeout(tick, 50);
      return;
    }
    const status = emu.runMs(elapsed * speed);
    busy += performance.now() - now;
    const audio = emu.takeAudio();
    // Fast-forward plays silently: sped-up audio would only be noise.
    if (audio.length && speed === 1) post({ type: "audio", audio }, [audio.buffer]);
    const frame = emu.frame();
    post({ type: "frame", frame }, [frame.buffer]);
    if (now - lastStats > 2000) {
      lastStats = now;
      post({ type: "stats", virtual: emu.virtualSeconds(), wall: (now - t0) / 1000, busy: busy / 1000 });
    }
    const log = emu.takeLog();
    if (log) post({ type: "log", text: log });
    if (status === 0) timer = setTimeout(tick, 1000 / 70);
    else post(status === 1 ? { type: "exit" } : { type: "error", message: log });
  };
  tick();
}

self.onmessage = async (e: MessageEvent<ToWorker>) => {
  const m = e.data;
  switch (m.type) {
    case "start": {
      await init();
      emu = new Emu(new Uint8Array(m.exe), m.dat, m.cmdline, (name: string, data: Uint8Array) =>
        post({ type: "save", name, data }),
      );
      for (const s of m.saves) emu.putFile(s.name, s.data);
      emu.setVolume(m.music, m.voices);
      emu.setBatterySaver(m.batterySaver);
      if (m.snapshot) {
        try {
          emu.loadState(m.snapshot);
          post({ type: "loaded", ok: true });
        } catch (err) {
          post({ type: "loaded", ok: false, message: String(err) });
        }
      }
      post({ type: "started" });
      clearTimeout(timer);
      loop();
      break;
    }
    case "key":
      emu?.key(m.code, m.pressed);
      break;
    case "mouse":
      emu?.mouse(m.x, m.y, m.buttons);
      break;
    case "pause":
      paused = m.paused;
      break;
    case "speed":
      speed = m.factor;
      break;
    case "volume":
      emu?.setVolume(m.music, m.voices);
      break;
    case "batterySaver":
      emu?.setBatterySaver(m.on);
      break;
    case "snapshot":
      if (emu) {
        const data = emu.saveState();
        post({ type: "snapshot", id: m.id, data }, [data.buffer]);
      }
      break;
    case "load":
      if (emu) {
        try {
          emu.loadState(m.data);
          post({ type: "loaded", ok: true });
        } catch (err) {
          post({ type: "loaded", ok: false, message: String(err) });
        }
      }
      break;
  }
};
