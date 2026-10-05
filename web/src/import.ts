// Import the game files from whatever the user has: the two files themselves,
// a CD image (.iso, ISO 9660, 2048-byte sectors), or a .zip containing either.
// Everything happens locally with Blob slices; large files stream into OPFS.

import { type GameFileName, REQUIRED, scratchFile, store } from "./storage";

/** SHA-256 of the DNCDPRG.EXE this engine targets (Dune CD 3.7). */
export const EXE_SHA256 = "5f30aeb84d67cf2e053a83c09c2890f010f2e25ee877ebec58ea15c5b30cfff9";

export type Progress = (label: string, fraction: number) => void;
type Found = Partial<Record<GameFileName, Blob>>;

const u16 = (b: Uint8Array, o: number) => b[o] | (b[o + 1] << 8);
const u32 = (b: Uint8Array, o: number) => (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
const bytes = async (blob: Blob, start: number, end: number) => new Uint8Array(await blob.slice(start, end).arrayBuffer());
const isWanted = (name: string): name is GameFileName => (REQUIRED as readonly string[]).includes(name);

/** Find DNCDPRG.EXE and DUNE.DAT inside an ISO 9660 image. */
export async function scanIso(iso: Blob): Promise<Found> {
  const SECTOR = 2048;
  const pvd = await bytes(iso, 16 * SECTOR, 17 * SECTOR);
  if (pvd[0] !== 1 || new TextDecoder().decode(pvd.subarray(1, 6)) !== "CD001") {
    throw new Error("This disc image is not a plain ISO 9660 image (BIN/CUE and other formats are not supported).");
  }
  const found: Found = {};
  const walk = async (lba: number, size: number, depth: number) => {
    const dir = await bytes(iso, lba * SECTOR, lba * SECTOR + size);
    let o = 0;
    while (o < dir.length) {
      const len = dir[o];
      if (len === 0) {
        o = (Math.floor(o / SECTOR) + 1) * SECTOR; // records never straddle sectors
        continue;
      }
      const extent = u32(dir, o + 2);
      const dataLen = u32(dir, o + 10);
      const isDir = (dir[o + 25] & 2) !== 0;
      const nameLen = dir[o + 32];
      const raw = new TextDecoder().decode(dir.subarray(o + 33, o + 33 + nameLen));
      const name = raw.split(";")[0].toUpperCase();
      if (nameLen === 1 && (dir[o + 33] === 0 || dir[o + 33] === 1)) {
        // "." and ".."
      } else if (isDir) {
        if (depth < 4) await walk(extent, dataLen, depth + 1);
      } else if (isWanted(name) && !found[name]) {
        found[name] = iso.slice(extent * SECTOR, extent * SECTOR + dataLen);
      }
      o += len;
    }
  };
  await walk(u32(pvd, 156 + 2), u32(pvd, 156 + 10), 0);
  return found;
}

export interface ZipEntry {
  name: string;
  method: number;
  compressed: number;
  size: number;
  localOffset: number;
}

export async function zipEntries(zip: Blob): Promise<ZipEntry[]> {
  const tailStart = Math.max(0, zip.size - 65557);
  const tail = await bytes(zip, tailStart, zip.size);
  let eocd = -1;
  for (let i = tail.length - 22; i >= 0; i--) {
    if (u32(tail, i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error("Not a ZIP archive.");
  const count = u16(tail, eocd + 10);
  const cdSize = u32(tail, eocd + 12);
  const cdOffset = u32(tail, eocd + 16);
  if (cdOffset === 0xffffffff) throw new Error("ZIP64 archives are not supported; please extract it first.");
  const cd = await bytes(zip, cdOffset, cdOffset + cdSize);
  const out: ZipEntry[] = [];
  let o = 0;
  for (let i = 0; i < count && u32(cd, o) === 0x02014b50; i++) {
    const nameLen = u16(cd, o + 28);
    const name = new TextDecoder().decode(cd.subarray(o + 46, o + 46 + nameLen));
    out.push({
      name,
      method: u16(cd, o + 10),
      compressed: u32(cd, o + 20),
      size: u32(cd, o + 24),
      localOffset: u32(cd, o + 42),
    });
    o += 46 + nameLen + u16(cd, o + 30) + u16(cd, o + 32);
  }
  return out;
}

/** The bytes of a stored (uncompressed) ZIP entry, or null if compressed. */
export async function storedEntry(zip: Blob, e: ZipEntry): Promise<Blob | null> {
  if (e.method !== 0) return null;
  const local = await bytes(zip, e.localOffset, e.localOffset + 30);
  const dataStart = e.localOffset + 30 + u16(local, 26) + u16(local, 28);
  return zip.slice(dataStart, dataStart + e.size);
}

/** Stream one ZIP entry's contents into a scratch OPFS file. */
export async function unzipEntry(zip: Blob, e: ZipEntry, progress: Progress): Promise<Blob> {
  const local = await bytes(zip, e.localOffset, e.localOffset + 30);
  const dataStart = e.localOffset + 30 + u16(local, 26) + u16(local, 28);
  let stream = zip.slice(dataStart, dataStart + e.compressed).stream();
  if (e.method === 8) stream = stream.pipeThrough(new DecompressionStream("deflate-raw"));
  else if (e.method !== 0) throw new Error(`${e.name}: unsupported compression (only stored/deflate).`);
  const base = e.name.split("/").pop() ?? e.name;
  return scratchFile(base, stream, e.size, (f) => progress(`Unpacking ${base}`, f));
}

/** Locate the game files in whatever the user picked. */
async function locate(files: File[], progress: Progress): Promise<Found> {
  const found: Found = {};
  for (const f of files) {
    const name = f.name.toUpperCase();
    if (isWanted(name)) {
      found[name] = f;
    } else if (name.endsWith(".ISO")) {
      progress(`Reading ${f.name}`, 0);
      Object.assign(found, await scanIso(f));
    } else if (name.endsWith(".ZIP")) {
      progress(`Reading ${f.name}`, 0);
      for (const e of await zipEntries(f)) {
        const base = (e.name.split("/").pop() ?? "").toUpperCase();
        if (isWanted(base) && !found[base]) found[base] = await unzipEntry(f, e, progress);
        else if (base.endsWith(".ISO")) Object.assign(found, await scanIso(await unzipEntry(f, e, progress)));
      }
    }
  }
  return found;
}

export async function sha256(blob: Blob): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", await blob.arrayBuffer());
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Import into OPFS; returns the stored files (or in-memory ones if storage fails). */
export async function importGameFiles(files: File[], progress: Progress): Promise<Found> {
  const found = await locate(files, progress);
  const out: Found = {};
  for (const n of REQUIRED) {
    const blob = found[n];
    if (!blob) continue;
    try {
      out[n] = await store(n, blob, (f) => progress(`Copying ${n} into browser storage`, f));
    } catch {
      out[n] = blob; // storage unavailable: use for this session only
    }
  }
  return out;
}
