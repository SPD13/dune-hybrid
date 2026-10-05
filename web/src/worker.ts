// Runs the emulated PC. The emulator never blocks, so a plain timer loop
// drives it: each tick catches virtual time up with wall time.
import init, { Emu } from "./pkg/web.js";

type In =
  | { type: "start"; exe: ArrayBuffer; dat: Blob; cmdline: string; saves: { name: string; data: Uint8Array }[] }
  | { type: "key"; code: number; pressed: boolean }
  | { type: "mouse"; x: number; y: number; buttons: number };

let emu: Emu | null = null;
const post = (msg: unknown, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(msg, transfer);

function loop() {
  if (!emu) return;
  let last = performance.now();
  const t0 = last;
  let lastStats = last;
  let busy = 0;
  const tick = () => {
    const now = performance.now();
    // Cap catch-up so a throttled tab does not run a huge burst.
    const status = emu!.runMs(Math.min(now - last, 100));
    busy += performance.now() - now;
    last = now;
    const audio = emu!.takeAudio();
    if (audio.length) post({ type: "audio", audio }, [audio.buffer]);
    const frame = emu!.frame();
    post({ type: "frame", frame }, [frame.buffer]);
    if (now - lastStats > 2000) {
      lastStats = now;
      post({ type: "stats", virtual: emu!.virtualSeconds(), wall: (now - t0) / 1000, busy: busy / 1000, instructions: emu!.instructions() });
    }
    const log = emu!.takeLog();
    if (log) post({ type: "log", text: log });
    if (status === 0) setTimeout(tick, 1000 / 70);
    else post({ type: status === 1 ? "exit" : "error", message: log });
  };
  tick();
}

self.onmessage = async (e: MessageEvent<In>) => {
  const m = e.data;
  if (m.type === "start") {
    await init();
    emu = new Emu(new Uint8Array(m.exe), m.dat, m.cmdline, (name: string, data: Uint8Array) =>
      post({ type: "save", name, data }),
    );
    for (const s of m.saves) emu.putFile(s.name, s.data);
    post({ type: "started" });
    loop();
  } else if (emu && m.type === "key") {
    emu.key(m.code, m.pressed);
  } else if (emu && m.type === "mouse") {
    emu.mouse(m.x, m.y, m.buttons);
  }
};
