// Title screen: game file import/status, Continue (resume point), New game,
// options. Then hands over to the play session.

import { datTocHash, forgetPack, importPack, packFile, packInfo } from "./hdpack";
import { EXE_SHA256, importGameFiles, sha256 } from "./import";
import { mountOptions } from "./options";
import { startSession } from "./session";
import { SONGS, forgetSoundtrack, importSoundtrack, loadManifest, segments } from "./soundtrack";
import { type GameFileName, REQUIRED, clearScratch, forgetGameFiles, getSnapshot, scratchFile, store, stored } from "./storage";

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
  refreshPack();
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

async function play(snapshot?: Uint8Array) {
  $("setup").hidden = true;
  startSession({
    exe: files["DNCDPRG.EXE"]!,
    dat: files["DUNE.DAT"]!,
    hdPack: (await packFile()) ?? undefined,
    snapshot,
    onExit: () => location.reload(),
  });
}

// ---- optional HD art pack ----
async function refreshPack() {
  const info = packInfo();
  const status = $("hdpack-status");
  $("hdpack-forget").hidden = !info;
  if (!info) {
    status.textContent = "Not imported: HD sprites and text use the built-in pixel-art upscaler.";
    return;
  }
  const mb = (info.bytes / 1e6).toFixed(0);
  status.textContent = `${info.sprites} sprites upscaled with ${info.model?.name ?? "?"} (${info.model?.license ?? "?"}), ${mb} MB. Turn on Options → HD sprites and text to use it.`;
  const dat = files["DUNE.DAT"];
  if (dat && info.toc && (await datTocHash(dat)) !== info.toc) {
    status.textContent += " Warning: it was made from a different DUNE.DAT, so most sprites will not match.";
  }
}

$<HTMLInputElement>("hdpack-input").addEventListener("change", async (e) => {
  const zip = (e.target as HTMLInputElement).files?.[0];
  if (!zip) return;
  const bar = $<HTMLProgressElement>("hdpack-progress");
  const msg = $("hdpack-message");
  msg.textContent = "";
  bar.hidden = false;
  try {
    await importPack(zip, (f) => (bar.value = f));
  } catch (err) {
    msg.textContent = String((err as Error).message ?? err);
  } finally {
    bar.hidden = true;
    refreshPack();
  }
});

$("hdpack-forget").addEventListener("click", async () => {
  await forgetPack();
  refreshPack();
});

$("btn-new").addEventListener("click", () => play());
$("btn-continue").addEventListener("click", async () => {
  const snap = await getSnapshot("auto");
  play(snap?.data);
});

// ---- optional remastered soundtrack ----
function refreshMusic() {
  const segs = segments(loadManifest());
  const list = $("music-songs");
  list.replaceChildren(
    ...Object.entries(SONGS).map(([id, name]) => {
      const li = document.createElement("li");
      li.textContent = name;
      li.className = segs.has(Number(id)) ? "ok" : "";
      li.title = segs.has(Number(id)) ? "Remastered recording" : "Original FM music";
      return li;
    }),
  );
  $("music-forget").hidden = segs.size === 0;
  $("music-status").textContent = segs.size
    ? `${segs.size} of 10 game songs use the remastered recordings (CRYOMUS, the Cryo logo jingle, has no remaster).`
    : "Not imported: the game uses its original AdLib/OPL3 music.";
}

$<HTMLInputElement>("music-input").addEventListener("change", async (e) => {
  const zip = (e.target as HTMLInputElement).files?.[0];
  if (!zip) return;
  const bar = $<HTMLProgressElement>("music-progress");
  const msg = $("music-message");
  msg.textContent = "";
  bar.hidden = false;
  try {
    await importSoundtrack(zip, (_label, f) => (bar.value = f));
  } catch (err) {
    msg.textContent = String((err as Error).message ?? err);
  } finally {
    bar.hidden = true;
    clearScratch().catch(() => {});
    refreshMusic();
refreshPack();
  }
});

$("music-forget").addEventListener("click", async () => {
  await forgetSoundtrack();
  refreshMusic();
});

refreshMusic();

if (import.meta.env.DEV && new URLSearchParams(location.search).has("devmusic") && !loadManifest()) {
  try {
    const blob = await (await fetch("dev-soundtrack.zip")).blob();
    await importSoundtrack(new File([blob], "soundtrack.zip"), () => {});
  } catch (err) {
    console.warn("devmusic", err);
  }
  refreshMusic();
}

if (import.meta.env.DEV && new URLSearchParams(location.search).has("devpack") && !packInfo()) {
  try {
    // The pack file changes between runs of dune-hd: never use a cached
    // copy. Streamed to storage (packs are too big to hold as a Blob).
    const res = await fetch(`dev-hdpack.zip?t=${Date.now()}`);
    const file = await scratchFile("dev-hdpack.zip", res.body!, Number(res.headers.get("content-length")), () => {});
    await importPack(file, () => {});
    await clearScratch();
  } catch (err) {
    console.warn("devpack", err);
  }
  refreshPack();
}

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
