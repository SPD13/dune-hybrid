// The options form, shared by the title screen and the in-game menu.
// Fields are named after setting paths ("music", "graphics.scaler", …).

import { LANGUAGES, PRESETS, type Settings, loadSettings, presetOf, saveSettings } from "./settings";

type Field = HTMLInputElement | HTMLSelectElement;
type Bag = Record<string, unknown>;

const get = (s: Bag, path: string) => path.split(".").reduce<unknown>((o, k) => (o as Bag | undefined)?.[k], s);

function set(s: Bag, path: string, value: unknown) {
  const keys = path.split(".");
  const last = keys.pop()!;
  (keys.reduce((o, k) => o[k] as Bag, s) as Bag)[last] = value;
}

const read = (el: Field): unknown =>
  el instanceof HTMLInputElement && el.type === "checkbox" ? el.checked : el instanceof HTMLInputElement && el.type === "range" ? Number(el.value) : el.value;

/** Fill `form` from the template and keep settings in sync with it. */
export function mountOptions(form: HTMLElement, onChange: (s: Settings) => void) {
  const tpl = document.getElementById("options-template") as HTMLTemplateElement;
  form.replaceChildren(tpl.content.cloneNode(true));
  const lang = form.querySelector<HTMLSelectElement>('select[name="language"]')!;
  for (const [code, name] of Object.entries(LANGUAGES)) lang.add(new Option(name, code));
  const fields = () => Array.from(form.querySelectorAll<Field>("[name]"));

  const sync = () => {
    const s = loadSettings() as unknown as Bag;
    for (const el of fields()) {
      const v = get(s, el.name);
      if (v === undefined) continue;
      if (el instanceof HTMLInputElement && el.type === "checkbox") el.checked = Boolean(v);
      else el.value = String(v);
    }
    const g = (s as unknown as Settings).graphics;
    form.querySelectorAll<HTMLElement>("[data-when-crt]").forEach((el) => (el.hidden = g.crt === "off"));
    form.querySelectorAll<HTMLElement>("[data-when-xbr]").forEach((el) => (el.hidden = g.scaler !== "xbr" || g.crt === "lottes"));
    // The CRT shader works from the original picture with its own resampling.
    for (const n of ["graphics.scaler", "graphics.output"]) form.querySelector<Field>(`[name="${n}"]`)!.disabled = g.crt === "lottes";
  };

  form.addEventListener("input", (e) => {
    const s = loadSettings();
    for (const el of fields()) set(s as unknown as Bag, el.name, read(el));
    const changed = (e.target as Field).name;
    const g = s.graphics;
    if (changed === "graphics.preset" && g.preset !== "custom") Object.assign(g, PRESETS[g.preset]);
    else if (changed.startsWith("graphics.")) g.preset = presetOf(g);
    saveSettings(s);
    sync();
    onChange(s);
  });
  sync();
  return { sync };
}
