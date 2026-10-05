// User preferences (per browser), kept in localStorage.

export type Language = "ENG" | "FRA" | "GER" | "ITA" | "SPA" | "DUT";
export type TouchMode = "direct" | "trackpad";
export type Display = "sharp" | "smooth" | "crt";
export type Buttons = "auto" | "always" | "never";

export interface Settings {
  language: Language;
  music: number; // 0..1
  voices: number; // 0..1
  display: Display;
  touchMode: TouchMode;
  buttons: Buttons;
  batterySaver: boolean;
}

export const DEFAULTS: Settings = {
  language: "ENG",
  music: 0.8,
  voices: 1,
  display: "sharp",
  touchMode: "direct",
  buttons: "auto",
  batterySaver: true,
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
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return { ...DEFAULTS };
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
