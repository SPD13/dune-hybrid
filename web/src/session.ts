// One play session: the emulator worker, rendering, audio, input, on-screen
// controls, the pause menu with quick save slots, and the automatic resume
// point (snapshotted every 30 s and whenever the page is hidden).

import { Input } from "./input";
import { mountOptions } from "./options";
import { createRenderer, thumbnail as frameThumbnail } from "./render";
import { commandLine, hdScale, hdVisible, isTouchDevice, loadSettings } from "./settings";
import { SoundtrackPlayer, loadManifest, replacedMask, segments } from "./soundtrack";
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
  /** HD art pack (stored ZIP), if imported. */
  hdPack?: Blob;
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
  // Remastered soundtrack (optional, imported by the player).
  const soundtrack = segments(loadManifest());
  let remaster: SoundtrackPlayer | null = null;
  try {
    const ctx = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
    await ctx.audioWorklet.addModule(new URL("audio-worklet.js", document.baseURI));
    audioNode = new AudioWorkletNode(ctx, "dune-audio", { numberOfInputs: 0, outputChannelCount: [2] });
    audioNode.connect(ctx.destination);
    if (soundtrack.size) remaster = await SoundtrackPlayer.create(ctx, soundtrack);
    if (import.meta.env.DEV) Object.assign(window, { __remaster: remaster, __audioCtx: ctx });
    const resume = () => ctx.state !== "running" && ctx.resume().catch(() => {});
    resume();
    for (const ev of ["pointerdown", "keydown"]) window.addEventListener(ev, resume);
  } catch (err) {
    console.warn("audio unavailable", err);
  }

  // ---- rendering ----
  const touch = isTouchDevice();
  const renderer = createRenderer(canvas, stage, touch);
  if (import.meta.env.DEV) Object.assign(window, { __renderer: renderer, __send: send });
  let pending: Uint8Array | null = null;
  let lastFrame: Uint8Array | null = null;
  if (import.meta.env.DEV) {
    Object.assign(window, {
      __lab: async (...args: unknown[]) => {
        const { openLab } = await import("./render/lab");
        if (lastFrame) (openLab as (...a: unknown[]) => void)(lastFrame, ...args);
      },
    });
  }
  const draw = () => {
    if (pending) {
      lastFrame = pending;
      pending = null;
      renderer.frame(lastFrame);
    }
    requestAnimationFrame(draw);
  };
  requestAnimationFrame(draw);

  const fit = () => {
    // 320×200 shown at the original 4:3 aspect, as large as the stage allows.
    const box = $("viewport").getBoundingClientRect();
    const scale = Math.min(box.width / 320, box.height / 240);
    const w = Math.floor(320 * scale);
    const h = Math.floor(240 * scale);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    // Render at device resolution; phones are capped at 2× to save power.
    renderer.resize(w, h, Math.min(devicePixelRatio || 1, touch ? 2 : 3));
    placeCompare();
  };

  // ---- compare mode: original rendering right of a draggable divider ----
  let compare: number | null = null;
  const compareBox = $("compare");
  const divider = compareBox.querySelector<HTMLElement>(".compare-divider")!;
  const compareButton = $("btn-compare");
  compareButton.hidden = renderer.kind !== "webgl2";
  const placeCompare = () => {
    if (compare === null) return;
    Object.assign(compareBox.style, {
      left: `${canvas.offsetLeft}px`,
      top: `${canvas.offsetTop}px`,
      width: `${canvas.offsetWidth}px`,
      height: `${canvas.offsetHeight}px`,
    });
    divider.style.left = `${compare * 100}%`;
    divider.setAttribute("aria-valuenow", String(Math.round(compare * 100)));
  };
  const setCompare = (split: number | null) => {
    compare = split === null ? null : Math.min(1, Math.max(0, split));
    compareBox.hidden = compare === null;
    compareButton.classList.toggle("active", compare !== null);
    compareButton.setAttribute("aria-pressed", String(compare !== null));
    renderer.setCompare(compare);
    placeCompare();
  };
  compareButton.onclick = () => setCompare(compare === null ? 0.5 : null);
  divider.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      divider.setPointerCapture(e.pointerId);
    } catch {
      // synthetic events
    }
  });
  divider.addEventListener("pointermove", (e) => {
    if (!divider.hasPointerCapture?.(e.pointerId) && e.buttons === 0) return;
    e.stopPropagation();
    const r = canvas.getBoundingClientRect();
    setCompare((e.clientX - r.left) / r.width);
  });
  divider.addEventListener("keydown", (e) => {
    const step = e.shiftKey ? 0.1 : 0.02;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      e.stopPropagation();
      setCompare((compare ?? 0.5) + (e.key === "ArrowLeft" ? -step : step));
    }
  });
  const remasterMask = () => (remaster && settings.remaster ? replacedMask(soundtrack) : 0);
  const applySettings = () => {
    renderer.setGraphics(settings.graphics);
    const showButtons = settings.buttons === "always" || (settings.buttons === "auto" && isTouchDevice());
    stage.classList.toggle("with-buttons", showButtons);
    send({ type: "volume", music: settings.music, voices: settings.voices });
    send({ type: "replaced", mask: remasterMask() });
    remaster?.setUserVolume(settings.remaster ? settings.music : 0);
    if (settings.remaster) remaster?.resync();
    send({ type: "batterySaver", on: settings.batterySaver });
    // HD needs the GPU path.
    const gpu = renderer.kind === "webgl2";
    send({ type: "hd", scale: gpu ? hdScale(settings.graphics, touch) : 0, visible: gpu && hdVisible(settings.graphics) });
    requestAnimationFrame(fit);
  };
  window.addEventListener("resize", fit);
  if (import.meta.env.DEV) Object.assign(window, { __compare: (v: number | null) => setCompare(v) });
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
  const thumbnail = () => (lastFrame ? frameThumbnail(lastFrame) : "");
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
  let fastForward = false;
  const setPaused = (p: boolean) => {
    send({ type: "pause", paused: p });
    remaster?.setSuspended(p || fastForward);
  };
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
  $("options").querySelectorAll(".renderer-note").forEach((el) => {
    el.textContent = renderer.kind === "webgl2" ? "" : "This browser has no WebGL2: upscalers are unavailable and the monitor effects are simplified.";
  });

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
    fastForward = true;
    remaster?.setSuspended(true);
    ff.classList.add("active");
  };
  const ffOff = () => {
    send({ type: "speed", factor: 1 });
    fastForward = false;
    remaster?.setSuspended(menuOpen || document.hidden);
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
        if (m.hd) renderer.hd(m.hd);
        break;
      case "audio":
        audioNode?.port.postMessage(m.audio, [m.audio.buffer]);
        break;
      case "music":
        if (settings.remaster) remaster?.update(m.events, m.state);
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
      hdPack: opts.hdPack,
      cmdline: commandLine(settings),
      saves,
      snapshot: opts.snapshot,
      music: settings.music,
      voices: settings.voices,
      batterySaver: settings.batterySaver,
      replacedSongs: remasterMask(),
      hd: renderer.kind === "webgl2" ? hdScale(settings.graphics, touch) : 0,
      hdVisible: renderer.kind === "webgl2" && hdVisible(settings.graphics),
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
