// Title screen: game file import/status, Continue (resume point), New game,
// options. Then hands over to the play session.

import { EXE_SHA256, importGameFiles, sha256 } from "./import";
import { mountOptions } from "./options";
import { startSession } from "./session";
import { type GameFileName, REQUIRED, clearScratch, forgetGameFiles, getSnapshot, store, stored } from "./storage";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const files: Partial<Record<GameFileName, Blob>> = {};

function setProgress(label: string | null, fraction = 0) {
  const bar = $<HTMLProgressElement>("files-progress");
  const text = $("files-progress-label");
  bar.hidden = text.hidden = label === null;
  if (label !== null) {
    bar.value = fraction;
    text.textContent = label;
  }
}

async function refresh() {
  const missing = REQUIRED.filter((n) => !files[n]);
  const status = $("files-status");
  $<HTMLButtonElement>("btn-new").disabled = missing.length > 0;
  $("files-forget").hidden = missing.length === REQUIRED.length;
  if (missing.length) {
    status.textContent = missing.length === REQUIRED.length ? "No game files yet." : `Missing: ${missing.join(", ")}.`;
    $("resume-card").hidden = true;
    return;
  }
  const hash = await sha256(files["DNCDPRG.EXE"]!);
  status.textContent =
    hash === EXE_SHA256
      ? "Game files ready: Dune CD version 3.7."
      : "Game files ready. This DNCDPRG.EXE is not the CD 3.7 release this project is tested with; it may still work.";
  const snap = await getSnapshot("auto").catch(() => undefined);
  $("resume-card").hidden = !snap;
  if (snap) {
    $<HTMLImageElement>("resume-thumb").src = snap.thumbnail;
    $("resume-time").textContent = `Saved ${new Date(snap.time).toLocaleString()}`;
  }
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
      setProgress(`Importing dev copy of ${n}`);
      files[n] = await store(n, await (await fetch(`dev/${n}`)).blob(), (f) => setProgress(`Importing dev copy of ${n}`, f));
    }
    setProgress(null);
  }
  await refresh();
}

$<HTMLInputElement>("files-input").addEventListener("change", async (e) => {
  const picked = Array.from((e.target as HTMLInputElement).files ?? []);
  const message = $("message");
  message.textContent = "";
  try {
    const found = await importGameFiles(picked, (label, f) => setProgress(label, f));
    Object.assign(files, found);
    if (!found["DUNE.DAT"] && !found["DNCDPRG.EXE"]) {
      message.textContent = "No Dune CD files found in that selection.";
    }
  } catch (err) {
    message.textContent = String((err as Error).message ?? err);
  } finally {
    setProgress(null);
    clearScratch().catch(() => {});
    await refresh();
  }
});

$("files-forget").addEventListener("click", async () => {
  await forgetGameFiles();
  for (const n of REQUIRED) delete files[n];
  await refresh();
});

function play(snapshot?: Uint8Array) {
  $("setup").hidden = true;
  startSession({
    exe: files["DNCDPRG.EXE"]!,
    dat: files["DUNE.DAT"]!,
    snapshot,
    onExit: () => location.reload(),
  });
}

$("btn-new").addEventListener("click", () => play());
$("btn-continue").addEventListener("click", async () => {
  const snap = await getSnapshot("auto");
  play(snap?.data);
});

const optionsDialog = $("options-dialog");
const optionsForm = document.createElement("form");
optionsForm.onsubmit = () => false;
$("options-home").append(optionsForm);
const options = mountOptions(optionsForm, () => {});
$("btn-options").addEventListener("click", () => {
  options.sync();
  optionsDialog.hidden = false;
});
$("options-close").addEventListener("click", () => (optionsDialog.hidden = true));

// Offline support and "install to home screen" (production builds only).
if (!import.meta.env.DEV && "serviceWorker" in navigator) {
  navigator.serviceWorker.register("./sw.js").catch(() => {});
}

preflight();
