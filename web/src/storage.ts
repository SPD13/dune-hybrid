// Persistence. Game files live in the Origin Private File System (picked or
// imported once). Save games and machine snapshots live in IndexedDB.

export const REQUIRED = ["DNCDPRG.EXE", "DUNE.DAT"] as const;
export type GameFileName = (typeof REQUIRED)[number];

async function opfs(): Promise<FileSystemDirectoryHandle> {
  return navigator.storage.getDirectory();
}

export async function stored(name: GameFileName): Promise<File | null> {
  try {
    const file = await (await (await opfs()).getFileHandle(name)).getFile();
    return file.size > 0 ? file : null;
  } catch {
    return null;
  }
}

async function writeStream(name: string, stream: ReadableStream<Uint8Array>, size: number, onProgress: (f: number) => void): Promise<File> {
  const handle = await (await opfs()).getFileHandle(name, { create: true });
  const writable = await handle.createWritable();
  let done = 0;
  const counter = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      done += chunk.byteLength;
      if (size) onProgress(Math.min(1, done / size));
      controller.enqueue(chunk);
    },
  });
  await stream.pipeThrough(counter).pipeTo(writable);
  return handle.getFile();
}

/** Copy a game file into OPFS (streamed; DUNE.DAT is ~400 MB). */
export async function store(name: GameFileName, data: Blob, onProgress: (f: number) => void): Promise<File> {
  await navigator.storage.persist?.();
  return writeStream(name, data.stream(), data.size, onProgress);
}

/** Copy any file into OPFS under `name` (streamed). */
export async function storeAs(name: string, data: Blob, onProgress: (f: number) => void): Promise<File> {
  await navigator.storage.persist?.();
  return writeStream(name, data.stream(), data.size, onProgress);
}

/** An OPFS file by name, if present. */
export async function storedAs(name: string): Promise<File | null> {
  try {
    const file = await (await (await opfs()).getFileHandle(name)).getFile();
    return file.size > 0 ? file : null;
  } catch {
    return null;
  }
}

export async function removeStored(name: string): Promise<void> {
  await (await opfs()).removeEntry(name).catch(() => {});
}

/** Temporary OPFS file used while unpacking archives. */
export async function scratchFile(name: string, stream: ReadableStream<Uint8Array>, size: number, onProgress: (f: number) => void): Promise<File> {
  return writeStream(`scratch-${name}`, stream, size, onProgress);
}

export async function clearScratch(): Promise<void> {
  const dir = await opfs();
  for await (const name of (dir as unknown as { keys(): AsyncIterable<string> }).keys()) {
    if (name.startsWith("scratch-")) await dir.removeEntry(name).catch(() => {});
  }
}

export async function forgetGameFiles(): Promise<void> {
  const dir = await opfs();
  for (const name of REQUIRED) await dir.removeEntry(name).catch(() => {});
}

// ---- IndexedDB: save games and snapshots ----

export interface SaveFile {
  name: string;
  data: Uint8Array;
}

export interface Snapshot {
  /** "auto" (resume point) or "slot1".."slot3". */
  name: string;
  data: Uint8Array;
  time: number;
  /** Small JPEG data URL of the screen when taken. */
  thumbnail: string;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open("dune-hybrid", 2);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains("saves")) db.createObjectStore("saves", { keyPath: "name" });
      if (!db.objectStoreNames.contains("snapshots")) db.createObjectStore("snapshots", { keyPath: "name" });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function tx<T>(store: string, mode: IDBTransactionMode, op: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await openDb();
  try {
    return await new Promise<T>((resolve, reject) => {
      const t = db.transaction(store, mode);
      const req = op(t.objectStore(store));
      t.oncomplete = () => resolve(req.result);
      t.onerror = () => reject(t.error);
    });
  } finally {
    db.close();
  }
}

export const loadSaves = () => tx<SaveFile[]>("saves", "readonly", (s) => s.getAll());
export const putSave = (save: SaveFile) => tx("saves", "readwrite", (s) => s.put(save));
export const loadSnapshots = () => tx<Snapshot[]>("snapshots", "readonly", (s) => s.getAll());
export const getSnapshot = (name: string) => tx<Snapshot | undefined>("snapshots", "readonly", (s) => s.get(name));
export const putSnapshot = (snap: Snapshot) => tx("snapshots", "readwrite", (s) => s.put(snap));
export const deleteSnapshot = (name: string) => tx("snapshots", "readwrite", (s) => s.delete(name));
