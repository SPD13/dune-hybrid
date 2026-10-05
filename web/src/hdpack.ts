// Optional HD art pack, made by `dune-hd` from the player's own DUNE.DAT.
// The ZIP (stored entries) is copied into the browser's private storage
// (OPFS) and read in place by the worker: a4/<hash>.png and a2/<hash>.png
// per sprite, with a manifest.json describing it.

import { storedEntry, zipEntries } from "./import";
import { removeStored, storeAs, storedAs } from "./storage";

const FILE = "hd-pack.zip";
const INFO_KEY = "dune-hybrid-hdpack";

export interface PackInfo {
  sprites: number;
  model: { name: string; license: string; source: string };
  generator: string;
  /** Hash of the DUNE.DAT table of contents the pack was made from. */
  toc: string;
  imported: number;
  bytes: number;
}

export function packInfo(): PackInfo | null {
  try {
    return JSON.parse(localStorage.getItem(INFO_KEY) ?? "null");
  } catch {
    return null;
  }
}

export async function packFile(): Promise<File | null> {
  return packInfo() ? storedAs(FILE) : null;
}

/** Import a pack ZIP; throws with a readable message if it is not one. */
export async function importPack(zip: File, progress: (f: number) => void): Promise<PackInfo> {
  const entries = await zipEntries(zip);
  const m = entries.find((e) => e.name === "manifest.json");
  const data = m && (await storedEntry(zip, m));
  if (!data) throw new Error("This ZIP is not an HD art pack made by dune-hd (no manifest.json).");
  const manifest = JSON.parse(await data.text());
  if (manifest.format !== "dune-hybrid-hd-pack") throw new Error("This ZIP is not an HD art pack made by dune-hd.");
  if (manifest.version !== 2) {
    throw new Error(`This HD art pack has format version ${manifest.version}; this app reads version 2. Please make it again with the current dune-hd.`);
  }
  if (entries.some((e) => e.name.endsWith(".png") && e.method !== 0)) {
    throw new Error("The pack's images are compressed inside the ZIP; please use the ZIP exactly as dune-hd wrote it.");
  }
  await storeAs(FILE, zip, progress);
  const info: PackInfo = {
    sprites: manifest.assets?.length ?? 0,
    model: manifest.model,
    generator: manifest.generator,
    toc: manifest.game?.toc ?? "",
    imported: Date.now(),
    bytes: zip.size,
  };
  localStorage.setItem(INFO_KEY, JSON.stringify(info));
  return info;
}

export async function forgetPack(): Promise<void> {
  await removeStored(FILE);
  localStorage.removeItem(INFO_KEY);
}

/** The same hash dune-hd records (`hdpack::catalog::toc_hash`): FNV-1a 64
 * of "NAME:size:offset;" for each named entry of DUNE.DAT's table. */
export async function datTocHash(dat: Blob): Promise<string> {
  const head = new DataView(await dat.slice(0, 2).arrayBuffer());
  const count = head.getUint16(0, true);
  const toc = new Uint8Array(await dat.slice(2, 2 + count * 25).arrayBuffer());
  let text = "";
  for (let i = 0; i < count; i++) {
    const e = toc.subarray(i * 25, i * 25 + 25);
    let name = "";
    for (let k = 0; k < 16 && e[k]; k++) name += String.fromCharCode(e[k]);
    if (!name) continue;
    const dv = new DataView(e.buffer, e.byteOffset, 25);
    text += `${name}:${dv.getUint32(16, true)}:${dv.getUint32(20, true)};`;
  }
  let h = 0xcbf29ce484222325n;
  for (const c of new TextEncoder().encode(text)) {
    h ^= BigInt(c);
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return h.toString(16).padStart(16, "0");
}
