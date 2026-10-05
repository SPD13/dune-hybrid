// Optional remastered soundtrack: Stéphane Picq's "Dune Spice Opera 2024
// remaster" (purchased by the player on Bandcamp). Its tracks 13-20 are the
// game's own songs, labelled with the game's internal names (PC_ARRAKIS, …),
// re-rendered at the original tempo — so they can replace the FM music in sync.
//
// Import keeps only those tracks, in the browser's private storage (OPFS).

import { type ZipEntry, storedEntry, unzipEntry, zipEntries } from "./import";

export const BANDCAMP_URL = "https://stphanepicq.bandcamp.com/album/dune-spice-opera-2024-remaster-lp";

/** The game's song numbers (as passed to its music driver) and names. */
export const SONGS: Record<number, string> = {
  1: "SEKENCE",
  2: "WATER",
  3: "WORMSUIT",
  4: "WORMINTR",
  5: "WARSONG",
  6: "MORNING",
  7: "SIETCHM",
  8: "BAGDAD",
  9: "ARRAKIS",
  10: "CRYOMUS",
};

/** Album track label → game songs it contains. */
const LABELS: Record<string, number[]> = {
  PC_SEKENCE: [1],
  PC_WATER: [2],
  "PC_WORMINTR+WORMSUIT": [4, 3],
  PC_WARSONG: [5],
  PC_MORNING: [6],
  PC_SIETCH: [7],
  PC_BAGDAD: [8],
  PC_ARRAKIS: [9],
};

/**
 * The combined worm track holds WORMINTR then WORMSUIT with no gap. The game's
 * song data puts WORMINTR at 172.1 s (tools/song-length.py); the remaster's
 * transition (energy dip) is at 172.4 s. Applied only to a file of the
 * expected length (6:24).
 */
const WORM_SPLIT_S = 172.4;
const WORM_TRACK_S = 384;

export interface Segment {
  file: string; // OPFS file name
  start: number; // seconds
  end: number; // seconds (loop/end point)
}

interface Manifest {
  tracks: Record<string, { file: string; duration: number }>;
  imported: number;
}

const MANIFEST_KEY = "dune-hybrid-soundtrack";

export function loadManifest(): Manifest | null {
  try {
    const m = JSON.parse(localStorage.getItem(MANIFEST_KEY) ?? "null");
    return m && m.tracks ? m : null;
  } catch {
    return null;
  }
}

/** Game song number → segment of a stored recording. */
export function segments(m: Manifest | null): Map<number, Segment> {
  const out = new Map<number, Segment>();
  if (!m) return out;
  for (const [label, t] of Object.entries(m.tracks)) {
    const songs = LABELS[label];
    if (!songs) continue;
    if (songs.length === 2) {
      if (Math.abs(t.duration - WORM_TRACK_S) < 4) {
        out.set(songs[0], { file: t.file, start: 0, end: WORM_SPLIT_S });
        out.set(songs[1], { file: t.file, start: WORM_SPLIT_S, end: t.duration });
      } else {
        out.set(songs[0], { file: t.file, start: 0, end: t.duration });
      }
    } else {
      out.set(songs[0], { file: t.file, start: 0, end: t.duration });
    }
  }
  return out;
}

/** Bitmask of replaced songs, for the emulator (bit n = song n). */
export const replacedMask = (segs: Map<number, Segment>) => [...segs.keys()].reduce((m, s) => m | (1 << s), 0);

function labelOf(entryName: string): string | null {
  const m = entryName.toUpperCase().match(/PC_[A-Z]+(\+[A-Z]+)?/);
  return m && LABELS[m[0]] ? m[0] : null;
}

function duration(blob: Blob): Promise<number> {
  return new Promise((resolve) => {
    const a = new Audio();
    const url = URL.createObjectURL(blob);
    a.preload = "metadata";
    a.onloadedmetadata = () => {
      URL.revokeObjectURL(url);
      resolve(a.duration);
    };
    a.onerror = () => {
      URL.revokeObjectURL(url);
      resolve(0);
    };
    a.src = url;
  });
}

async function opfs() {
  return navigator.storage.getDirectory();
}

/** Import the purchased album ZIP; returns how many game songs it covers. */
export async function importSoundtrack(zip: File, progress: (label: string, f: number) => void): Promise<number> {
  const entries = (await zipEntries(zip)).filter((e: ZipEntry) => labelOf(e.name) && /\.(mp3|flac|ogg|m4a|wav)$/i.test(e.name));
  if (!entries.length) throw new Error("No game tracks (PC_…) found in this ZIP. Is it the Dune Spice Opera 2024 remaster download?");
  await navigator.storage.persist?.();
  const dir = await opfs();
  const tracks: Manifest["tracks"] = {};
  let i = 0;
  for (const e of entries) {
    const label = labelOf(e.name)!;
    progress(`Importing ${label}`, i++ / entries.length);
    const blob = (await storedEntry(zip, e)) ?? (await unzipEntry(zip, e, () => {}));
    const ext = e.name.split(".").pop()!.toLowerCase();
    const file = `music-${label}.${ext}`;
    const handle = await dir.getFileHandle(file, { create: true });
    const w = await handle.createWritable();
    await blob.stream().pipeTo(w);
    tracks[label] = { file, duration: await duration(await handle.getFile()) };
  }
  localStorage.setItem(MANIFEST_KEY, JSON.stringify({ tracks, imported: Date.now() } satisfies Manifest));
  return segments(loadManifest()).size;
}

export async function forgetSoundtrack(): Promise<void> {
  const m = loadManifest();
  const dir = await opfs();
  for (const t of Object.values(m?.tracks ?? {})) await dir.removeEntry(t.file).catch(() => {});
  localStorage.removeItem(MANIFEST_KEY);
}

async function file(name: string): Promise<File | null> {
  try {
    return await (await (await opfs()).getFileHandle(name)).getFile();
  } catch {
    return null;
  }
}

/** Ticks of the game's 200 Hz music timer. */
const TICK_S = 0x1745 / 1193182;
/** The game's normal music level (its fades move between 0, ~140 and 230). */
const FULL_VOLUME = 230;
/** FM silent this long while a song "plays" means it ended on its own. */
const ENDED_QUIET_MS = 4000;

/**
 * Plays the remastered recordings in place of the FM music, driven by the
 * game's own music calls (play / stop / fade) reported by the emulator.
 */
export class SoundtrackPlayer {
  private el = new Audio();
  private gain: GainNode;
  private urls = new Map<string, string>();
  private song = 0;
  private seg: Segment | null = null;
  private level = 1; // game-side volume (fades), 0..~1.2
  private userVolume = 1;
  private suspended = false;

  constructor(
    private ctx: AudioContext,
    private segs: Map<number, Segment>,
  ) {
    this.el.preload = "auto";
    const src = ctx.createMediaElementSource(this.el);
    this.gain = ctx.createGain();
    src.connect(this.gain).connect(ctx.destination);
    // Loop within the segment (the game loops songs; the worm track holds two).
    this.el.addEventListener("timeupdate", () => this.checkEnd());
    this.el.addEventListener("ended", () => this.checkEnd(true));
  }

  static async create(ctx: AudioContext, segs: Map<number, Segment>) {
    const p = new SoundtrackPlayer(ctx, segs);
    for (const s of segs.values()) {
      if (p.urls.has(s.file)) continue;
      const f = await file(s.file);
      if (f) p.urls.set(s.file, URL.createObjectURL(f));
    }
    return p;
  }

  /** Forget which song we think is playing, so the next update resyncs. */
  resync() {
    this.song = -1;
  }

  setUserVolume(v: number) {
    this.userVolume = v;
    this.applyGain(0.05);
  }

  /** Pause while the game is paused, hidden or fast-forwarding. */
  setSuspended(s: boolean) {
    this.suspended = s;
    if (s) this.el.pause();
    else if (this.seg) this.el.play().catch(() => {});
  }

  private applyGain(seconds: number) {
    const g = this.gain.gain;
    const now = this.ctx.currentTime;
    g.cancelScheduledValues(now);
    g.setValueAtTime(g.value, now);
    g.linearRampToValueAtTime(this.level * this.userVolume, now + Math.max(0.01, seconds));
  }

  private checkEnd(ended = false) {
    if (!this.seg) return;
    if (ended || this.el.currentTime >= this.seg.end - 0.05) {
      this.el.currentTime = this.seg.start;
      if (!this.suspended) this.el.play().catch(() => {});
    }
  }

  private start(song: number) {
    const seg = this.segs.get(song);
    const url = seg && this.urls.get(seg.file);
    this.song = song;
    if (!seg || !url) {
      this.stop();
      return;
    }
    this.seg = seg;
    if (this.el.src !== url) this.el.src = url;
    this.el.currentTime = seg.start;
    this.level = 1;
    this.applyGain(0.02);
    if (!this.suspended) this.el.play().catch(() => {});
  }

  private stop(fade = 0.15) {
    this.seg = null;
    this.level = 0;
    this.applyGain(fade);
    const el = this.el;
    setTimeout(() => {
      if (!this.seg) el.pause();
    }, fade * 1000 + 50);
  }

  /** Events from Emu.takeMusicEvents and the state from Emu.musicState. */
  update(events: Uint32Array, state: Uint32Array) {
    for (let i = 0; i + 3 < events.length; i += 4) {
      const kind = events[i + 1];
      if (kind === 1) this.start(events[i + 2]);
      else if (kind === 2) {
        this.song = 0;
        if (this.seg) this.stop();
      } else if (kind === 4 && this.seg) {
        this.level = Math.min(1.2, events[i + 3] / FULL_VOLUME);
        this.applyGain(events[i + 2] * TICK_S);
      }
    }
    const [song, status, , quietMs] = state;
    // Resync after a snapshot restore: the game is playing a song we are not.
    if (song !== this.song && song !== 0 && status & 0x80) this.start(song);
    // The game's rendition ended by itself (silent for a while): follow it.
    if (this.seg && quietMs > ENDED_QUIET_MS) this.stop(1.0);
  }
}
