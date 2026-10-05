import { SCANCODES } from "./keymap";
import { REQUIRED, forget, loadSaves, putSave, store, stored } from "./storage";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const setup = $("setup");
const status = $("files-status");
const input = $<HTMLInputElement>("files-input");
const forgetBtn = $<HTMLButtonElement>("files-forget");
const progress = $<HTMLProgressElement>("files-progress");
const play = $<HTMLButtonElement>("play");
const message = $("message");
const stage = $("stage");
const canvas = $<HTMLCanvasElement>("screen");
const toast = $("toast");

const files: Partial<Record<(typeof REQUIRED)[number], File>> = {};

function showToast(text: string, ms = 0) {
  toast.textContent = text;
  toast.hidden = false;
  if (ms) setTimeout(() => (toast.hidden = true), ms);
}

function refresh() {
  const missing = REQUIRED.filter((n) => !files[n]);
  play.disabled = missing.length > 0;
  forgetBtn.hidden = missing.length === REQUIRED.length;
  status.textContent = missing.length ? `Missing: ${missing.join(", ")}.` : "Both game files are stored in this browser.";
}

async function preflight() {
  for (const n of REQUIRED) {
    const f = await stored(n);
    if (f) files[n] = f;
  }
  // Dev convenience: `?devfiles` imports the local game files (never bundled).
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("devfiles")) {
    for (const n of REQUIRED) {
      if (files[n]) continue;
      status.textContent = `Importing dev copy of ${n}…`;
      const blob = await (await fetch(`dev/${n}`)).blob();
      files[n] = await store(n, blob, () => {});
    }
  }
  refresh();
}

input.addEventListener("change", async () => {
  message.textContent = "";
  for (const f of Array.from(input.files ?? [])) {
    const name = f.name.toUpperCase() as (typeof REQUIRED)[number];
    if (!REQUIRED.includes(name)) continue;
    progress.hidden = false;
    status.textContent = `Copying ${name} into browser storage…`;
    try {
      files[name] = await store(name, f, (v) => (progress.value = v));
    } catch {
      files[name] = f; // storage unavailable: use for this session only
    }
    progress.hidden = true;
  }
  refresh();
});

forgetBtn.addEventListener("click", async () => {
  await forget();
  for (const n of REQUIRED) delete files[n];
  refresh();
});

function fitCanvas() {
  const scale = Math.min(window.innerWidth / 320, window.innerHeight / 240);
  canvas.style.width = `${Math.floor(320 * scale)}px`;
  canvas.style.height = `${Math.floor(240 * scale)}px`;
}

async function startAudio(): Promise<AudioWorkletNode | null> {
  try {
    const ctx = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
    await ctx.audioWorklet.addModule(new URL("audio-worklet.js", document.baseURI));
    const node = new AudioWorkletNode(ctx, "dune-audio", { numberOfInputs: 0, outputChannelCount: [2] });
    node.connect(ctx.destination);
    const resume = () => ctx.state !== "running" && ctx.resume().catch(() => {});
    resume();
    for (const ev of ["pointerdown", "keydown"]) window.addEventListener(ev, resume);
    return node;
  } catch (err) {
    console.warn("audio unavailable", err);
    return null;
  }
}

play.addEventListener("click", async () => {
  play.disabled = true;
  const audio = await startAudio();
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  const ctx = canvas.getContext("2d", { alpha: false })!;
  const image = ctx.createImageData(320, 200);
  const rgba = new Uint32Array(image.data.buffer);
  const lut = new Uint32Array(256);
  let pending: Uint8Array | null = null;

  worker.onmessage = (e) => {
    const m = e.data;
    if (m.type === "frame") pending = m.frame;
    else if (m.type === "audio") {
      if (import.meta.env.DEV) {
        const w = window as unknown as { __audioFrames?: number; __audioPeak?: number };
        w.__audioFrames = (w.__audioFrames ?? 0) + m.audio.length / 2;
        for (let i = 0; i < m.audio.length; i += 16) w.__audioPeak = Math.max(w.__audioPeak ?? 0, Math.abs(m.audio[i]));
      }
      audio?.port.postMessage(m.audio, [m.audio.buffer]);
    }
    else if (m.type === "save") {
      putSave({ name: m.name, data: m.data }).catch((err) => console.error("save failed", err));
      // The game rewrites DUNE37S0.SAV (its start state) on every launch.
      if (m.name !== "DUNE37S0.SAV") showToast(`Saved ${m.name}`, 2000);
    }
    else if (m.type === "started") toast.hidden = true;
    else if (m.type === "log") console.debug(m.text);
    else if (m.type === "stats") {
      (window as unknown as { __stats: unknown }).__stats = m;
    }
    else if (m.type === "exit") showToast("The game has ended. Reload the page to play again.");
    else if (m.type === "error") showToast(`The emulator stopped: ${m.message}`);
  };

  const draw = () => {
    if (pending) {
      const f = pending;
      pending = null;
      for (let i = 0; i < 256; i++) {
        const p = 64000 + i * 3;
        lut[i] = 0xff000000 | (f[p + 2] << 16) | (f[p + 1] << 8) | f[p];
      }
      for (let i = 0; i < 64000; i++) rgba[i] = lut[f[i]];
      ctx.putImageData(image, 0, 0);
    }
    requestAnimationFrame(draw);
  };
  requestAnimationFrame(draw);

  const toGame = (e: MouseEvent) => {
    const r = canvas.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(319, Math.floor(((e.clientX - r.left) / r.width) * 320))),
      y: Math.max(0, Math.min(199, Math.floor(((e.clientY - r.top) / r.height) * 200))),
    };
  };
  const sendMouse = (e: MouseEvent) => worker.postMessage({ type: "mouse", ...toGame(e), buttons: e.buttons & 7 });
  canvas.addEventListener("mousemove", sendMouse);
  canvas.addEventListener("mousedown", (e) => {
    canvas.focus();
    sendMouse(e);
  });
  window.addEventListener("mouseup", sendMouse);
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());
  const key = (pressed: boolean) => (e: KeyboardEvent) => {
    const code = SCANCODES[e.code];
    if (code === undefined) return;
    e.preventDefault();
    if (pressed && e.repeat) return;
    worker.postMessage({ type: "key", code, pressed });
  };
  window.addEventListener("keydown", key(true));
  window.addEventListener("keyup", key(false));

  const exe = await files["DNCDPRG.EXE"]!.arrayBuffer();
  const saves = await loadSaves().catch(() => []);
  worker.postMessage({ type: "start", exe, dat: files["DUNE.DAT"], cmdline: "ADP330 SBP2227", saves }, [exe]);
  setup.hidden = true;
  stage.hidden = false;
  showToast("Starting…");
  fitCanvas();
  window.addEventListener("resize", fitCanvas);
  canvas.focus();
});

preflight();
