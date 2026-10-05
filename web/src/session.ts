// One play session: the emulator worker, rendering, audio, input, on-screen
// controls, the pause menu with quick save slots, and the automatic resume
// point (snapshotted every 30 s and whenever the page is hidden).

import { Input } from "./input";
import { mountOptions } from "./options";
import { commandLine, isTouchDevice, loadSettings } from "./settings";
import { type Snapshot, loadSaves, loadSnapshots, putSave, putSnapshot } from "./storage";
import type { FromWorker, ToWorker } from "./worker";

const AUTOSAVE_MS = 30_000;
const SLOTS = ["slot1", "slot2", "slot3"];
const SC_ESC = 0x01;
const SC_ENTER = 0x1c;
const SC_SPACE = 0x39;
const SC_P = 0x19;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

export interface SessionOptions {
  exe: Blob;
  dat: Blob;
  snapshot?: Uint8Array;
  onExit: () => void;
}

export async function startSession(opts: SessionOptions) {
  let settings = loadSettings();
  const stage = $("stage");
  const canvas = $<HTMLCanvasElement>("screen");
  const toast = $("toast");
  const menu = $("menu");
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  const send = (m: ToWorker, transfer: Transferable[] = []) => worker.postMessage(m, transfer);

  const showToast = (text: string, ms = 2500) => {
    toast.textContent = text;
    toast.hidden = false;
    clearTimeout((showToast as unknown as { t?: number }).t);
    (showToast as unknown as { t?: number }).t = ms ? window.setTimeout(() => (toast.hidden = true), ms) : undefined;
  };

  // ---- audio ----
  let audioNode: AudioWorkletNode | null = null;
  try {
    const ctx = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
    await ctx.audioWorklet.addModule(new URL("audio-worklet.js", document.baseURI));
    audioNode = new AudioWorkletNode(ctx, "dune-audio", { numberOfInputs: 0, outputChannelCount: [2] });
    audioNode.connect(ctx.destination);
    const resume = () => ctx.state !== "running" && ctx.resume().catch(() => {});
    resume();
    for (const ev of ["pointerdown", "keydown"]) window.addEventListener(ev, resume);
  } catch (err) {
    console.warn("audio unavailable", err);
  }

  // ---- rendering ----
  const ctx2d = canvas.getContext("2d", { alpha: false })!;
  const image = ctx2d.createImageData(320, 200);
  const rgba = new Uint32Array(image.data.buffer);
  const lut = new Uint32Array(256);
  let pending: Uint8Array | null = null;
  const draw = () => {
    if (pending) {
      const f = pending;
      pending = null;
      for (let i = 0; i < 256; i++) {
        const p = 64000 + i * 3;
        lut[i] = 0xff000000 | (f[p + 2] << 16) | (f[p + 1] << 8) | f[p];
      }
      for (let i = 0; i < 64000; i++) rgba[i] = lut[f[i]];
      ctx2d.putImageData(image, 0, 0);
    }
    requestAnimationFrame(draw);
  };
  requestAnimationFrame(draw);

  const fit = () => {
    // 320×200 shown at the original 4:3 aspect, as large as the stage allows.
    const box = $("viewport").getBoundingClientRect();
    const scale = Math.min(box.width / 320, box.height / 240);
    canvas.style.width = `${Math.floor(320 * scale)}px`;
    canvas.style.height = `${Math.floor(240 * scale)}px`;
  };
  const applySettings = () => {
    stage.dataset.display = settings.display;
    const showButtons = settings.buttons === "always" || (settings.buttons === "auto" && isTouchDevice());
    stage.classList.toggle("with-buttons", showButtons);
    send({ type: "volume", music: settings.music, voices: settings.voices });
    send({ type: "batterySaver", on: settings.batterySaver });
    requestAnimationFrame(fit);
  };
  window.addEventListener("resize", fit);
  window.visualViewport?.addEventListener("resize", fit);

  // ---- input ----
  const input = new Input(canvas, send, () => settings.touchMode);
  input.attach();

  // ---- snapshots ----
  let nextId = 1;
  const waiting = new Map<number, (data: Uint8Array) => void>();
  const snapshot = () =>
    new Promise<Uint8Array>((resolve) => {
      const id = nextId++;
      waiting.set(id, resolve);
      send({ type: "snapshot", id });
    });
  const thumbnail = () => {
    const t = document.createElement("canvas");
    t.width = 160;
    t.height = 120;
    t.getContext("2d")!.drawImage(canvas, 0, 0, 160, 120);
    return t.toDataURL("image/jpeg", 0.7);
  };
  const saveSnapshot = async (name: string) => {
    const data = await snapshot();
    await putSnapshot({ name, data, time: Date.now(), thumbnail: thumbnail() });
  };
  let started = false;
  const autosave = () => {
    if (started && !menuOpen) saveSnapshot("auto").catch((e) => console.warn("autosave", e));
  };
  window.setInterval(autosave, AUTOSAVE_MS);

  // ---- pause / visibility ----
  let menuOpen = false;
  const setPaused = (p: boolean) => send({ type: "pause", paused: p });
  let wakeLock: { release(): Promise<void> } | null = null;
  const requestWakeLock = async () => {
    try {
      wakeLock = await (navigator as unknown as { wakeLock?: { request(t: string): Promise<{ release(): Promise<void> }> } }).wakeLock?.request("screen") ?? null;
    } catch {
      wakeLock = null;
    }
  };
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      // Phones may discard a background tab: keep the resume point fresh.
      if (started) saveSnapshot("auto").catch(() => {});
      setPaused(true);
    } else {
      if (!menuOpen) setPaused(false);
      requestWakeLock();
    }
  });
  window.addEventListener("pagehide", () => started && saveSnapshot("auto").catch(() => {}));

  // ---- menu ----
  const renderSlots = async () => {
    const snaps = new Map<string, Snapshot>((await loadSnapshots().catch(() => [])).map((s) => [s.name, s]));
    const list = $("slots");
    list.replaceChildren();
    SLOTS.forEach((name, i) => {
      const snap = snaps.get(name);
      const li = document.createElement("li");
      li.className = "slot";
      const img = document.createElement(snap ? "img" : "div");
      img.className = "thumb";
      if (snap) (img as HTMLImageElement).src = snap.thumbnail;
      const info = document.createElement("div");
      info.className = "slot-info";
      info.innerHTML = `<strong>Slot ${i + 1}</strong><span>${snap ? new Date(snap.time).toLocaleString() : "Empty"}</span>`;
      const save = document.createElement("button");
      save.textContent = "Save";
      save.onclick = async () => {
        await saveSnapshot(name);
        showToast(`Saved to slot ${i + 1}`);
        renderSlots();
      };
      const load = document.createElement("button");
      load.textContent = "Load";
      load.disabled = !snap;
      load.onclick = () => {
        if (!snap) return;
        send({ type: "load", data: snap.data });
        closeMenu();
      };
      li.append(img, info, save, load);
      list.append(li);
    });
  };
  const openMenu = () => {
    menuOpen = true;
    setPaused(true);
    syncOptionsForm();
    renderSlots();
    menu.hidden = false;
  };
  const closeMenu = () => {
    menuOpen = false;
    menu.hidden = true;
    setPaused(false);
    canvas.focus();
  };
  $("btn-menu").onclick = openMenu;
  $("menu-resume").onclick = closeMenu;
  $("menu-quit").onclick = async () => {
    await saveSnapshot("auto").catch(() => {});
    location.reload();
  };
  $("menu-fullscreen").onclick = () => toggleFullscreen();
  window.addEventListener("keydown", (e) => {
    if (e.code === "F1" || (e.code === "Escape" && menuOpen)) {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (menuOpen) closeMenu();
      else openMenu();
    }
  }, { capture: true });

  const options = mountOptions($("options"), (s) => {
    settings = s;
    applySettings();
  });
  const syncOptionsForm = options.sync;

  // ---- on-screen buttons ----
  const bindTap = (id: string, code: number) => ($(id).onclick = () => input.tapKey(code));
  bindTap("btn-esc", SC_ESC);
  bindTap("btn-enter", SC_ENTER);
  bindTap("btn-space", SC_SPACE);
  bindTap("btn-pause", SC_P);
  const ff = $("btn-ff");
  const ffOn = (e: Event) => {
    e.preventDefault();
    send({ type: "speed", factor: 4 });
    ff.classList.add("active");
  };
  const ffOff = () => {
    send({ type: "speed", factor: 1 });
    ff.classList.remove("active");
  };
  ff.addEventListener("pointerdown", ffOn);
  ff.addEventListener("pointerup", ffOff);
  ff.addEventListener("pointercancel", ffOff);
  ff.addEventListener("pointerleave", ffOff);
  $("btn-quicksave").onclick = async () => {
    await saveSnapshot("slot1");
    showToast("Quick saved (slot 1)");
  };
  $("btn-quickload").onclick = async () => {
    const snap = (await loadSnapshots()).find((s) => s.name === "slot1");
    if (!snap) return showToast("No quick save yet");
    send({ type: "load", data: snap.data });
  };
  $("btn-fullscreen").onclick = () => toggleFullscreen();

  // ---- worker messages ----
  worker.onmessage = (e: MessageEvent<FromWorker>) => {
    const m = e.data;
    switch (m.type) {
      case "frame":
        pending = m.frame;
        break;
      case "audio":
        audioNode?.port.postMessage(m.audio, [m.audio.buffer]);
        break;
      case "save":
        putSave({ name: m.name, data: m.data }).catch((err) => console.error("save failed", err));
        if (m.name !== "DUNE37S0.SAV") showToast(`Game saved (${m.name})`);
        break;
      case "snapshot":
        waiting.get(m.id)?.(m.data);
        waiting.delete(m.id);
        break;
      case "loaded":
        showToast(m.ok ? "Restored" : `Could not restore: ${m.message}`);
        break;
      case "started":
        started = true;
        toast.hidden = true;
        break;
      case "stats":
        if (import.meta.env.DEV) Object.assign(window, { __stats: m });
        break;
      case "log":
        console.debug(m.text);
        break;
      case "exit":
        started = false;
        showToast("The game has ended.", 0);
        setTimeout(opts.onExit, 1500);
        break;
      case "error":
        showToast(`The emulator stopped: ${m.message}`, 0);
        break;
    }
  };

  // ---- go ----
  stage.hidden = false;
  applySettings();
  showToast(opts.snapshot ? "Resuming…" : "Starting…", 0);
  const exe = await opts.exe.arrayBuffer();
  const saves = await loadSaves().catch(() => []);
  send(
    {
      type: "start",
      exe,
      dat: opts.dat,
      cmdline: commandLine(settings),
      saves,
      snapshot: opts.snapshot,
      music: settings.music,
      voices: settings.voices,
      batterySaver: settings.batterySaver,
    },
    [exe],
  );
  requestWakeLock();
  canvas.focus();
}

export function toggleFullscreen() {
  const doc = document as Document & { webkitFullscreenElement?: Element; webkitExitFullscreen?: () => void };
  const el = document.documentElement as HTMLElement & { webkitRequestFullscreen?: () => void };
  if (document.fullscreenElement || doc.webkitFullscreenElement) {
    (document.exitFullscreen?.bind(document) ?? doc.webkitExitFullscreen)?.();
  } else {
    const req = el.requestFullscreen?.bind(el) ?? el.webkitRequestFullscreen?.bind(el);
    Promise.resolve(req?.()).then(() => (screen.orientation as unknown as { lock?(o: string): Promise<void> }).lock?.("landscape")).catch(() => {});
  }
}
