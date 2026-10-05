// The user's game files live in the Origin Private File System so they are
// picked only once.

export const REQUIRED = ["DNCDPRG.EXE", "DUNE.DAT"] as const;

export async function stored(name: string): Promise<File | null> {
  try {
    const root = await navigator.storage.getDirectory();
    const file = await (await root.getFileHandle(name)).getFile();
    return file.size > 0 ? file : null;
  } catch {
    return null;
  }
}

export async function store(name: string, file: Blob, onProgress: (f: number) => void): Promise<File> {
  await navigator.storage.persist?.();
  const root = await navigator.storage.getDirectory();
  const handle = await root.getFileHandle(name, { create: true });
  const writable = await handle.createWritable();
  let done = 0;
  const progress = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      done += chunk.byteLength;
      onProgress(done / file.size);
      controller.enqueue(chunk);
    },
  });
  await file.stream().pipeThrough(progress).pipeTo(writable);
  return handle.getFile();
}

export async function forget(): Promise<void> {
  const root = await navigator.storage.getDirectory();
  for (const name of REQUIRED) await root.removeEntry(name).catch(() => {});
}

export interface SaveFile {
  name: string;
  data: Uint8Array;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open("dune-hybrid", 1);
    req.onupgradeneeded = () => req.result.createObjectStore("saves", { keyPath: "name" });
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function loadSaves(): Promise<SaveFile[]> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const req = db.transaction("saves").objectStore("saves").getAll();
    req.onsuccess = () => resolve(req.result as SaveFile[]);
    req.onerror = () => reject(req.error);
  });
}

export async function putSave(save: SaveFile): Promise<void> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction("saves", "readwrite");
    tx.objectStore("saves").put(save);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}
