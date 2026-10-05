// User preferences (per browser), kept in localStorage.

export type Language = "ENG" | "FRA" | "GER" | "ITA" | "SPA" | "DUT";
export type TouchMode = "direct" | "trackpad";
export type Buttons = "auto" | "always" | "never";

/** Picture filters (web/src/render): an optional pixel-art upscaler, the
 * resampling to the screen, and an optional CRT look. */
export type Scaler = "none" | "mmpx" | "epx" | "xbr";
export type Output = "nearest" | "sharp" | "smooth";
export type Crt = "off" | "scanlines" | "lottes";
export type Preset = "original" | "smooth" | "scanlines" | "crt" | "pixel" | "xbr" | "custom";

export interface Graphics {
  preset: Preset;
  scaler: Scaler;
  output: Output;
  crt: Crt;
  crtStrength: number; // 0..1
  /** How much of the xBR result to use (the rest is the original pixel). */
  xbrAmount: number; // 0..1
}

export const PRESETS: Record<Exclude<Preset, "custom">, Pick<Graphics, "scaler" | "output" | "crt">> = {
  original: { scaler: "none", output: "sharp", crt: "off" },
  smooth: { scaler: "none", output: "smooth", crt: "off" },
  scanlines: { scaler: "none", output: "sharp", crt: "scanlines" },
  crt: { scaler: "none", output: "sharp", crt: "lottes" },
  pixel: { scaler: "mmpx", output: "sharp", crt: "off" },
  xbr: { scaler: "xbr", output: "smooth", crt: "off" },
};

/** The preset matching these filters, or "custom". */
export function presetOf(g: Pick<Graphics, "scaler" | "output" | "crt">): Preset {
  for (const [name, p] of Object.entries(PRESETS)) {
    if (p.scaler === g.scaler && p.output === g.output && p.crt === g.crt) return name as Preset;
  }
  return "custom";
}

export interface Settings {
  language: Language;
  music: number; // 0..1
  voices: number; // 0..1
  graphics: Graphics;
  touchMode: TouchMode;
  buttons: Buttons;
  batterySaver: boolean;
  /** Use the imported remastered soundtrack instead of the FM music. */
  remaster: boolean;
}

export const DEFAULTS: Settings = {
  language: "ENG",
  music: 0.8,
  voices: 1,
  graphics: { preset: "original", ...PRESETS.original, crtStrength: 0.8, xbrAmount: 0.6 },
  touchMode: "direct",
  buttons: "auto",
  batterySaver: true,
  remaster: true,
};

export const LANGUAGES: Record<Language, string> = {
  ENG: "English",
  FRA: "Français",
  GER: "Deutsch",
  ITA: "Italiano",
  SPA: "Español",
  DUT: "Nederlands",
};

const KEY = "dune-hybrid-settings";

export function loadSettings(): Settings {
  try {
    const { display, ...stored } = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    // Before the filter pipeline: display = "sharp" | "smooth" | "crt".
    const legacy = display === "smooth" ? "smooth" : display === "crt" ? "scanlines" : "original";
    const graphics = { ...DEFAULTS.graphics, ...(stored.graphics ?? { preset: legacy, ...PRESETS[legacy] }) };
    return { ...DEFAULTS, ...stored, graphics };
  } catch {
    return { ...DEFAULTS, graphics: { ...DEFAULTS.graphics } };
  }
}

export function saveSettings(s: Settings) {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    // private mode: settings last for this session only
  }
}

/** The DOS command line: sound cards as in Cryogenic, plus the language. */
export function commandLine(s: Settings): string {
  return `ADP330 SBP2227 ${s.language}`;
}

/** True on devices whose primary pointer is a finger. */
export const isTouchDevice = () => matchMedia("(pointer: coarse)").matches;
