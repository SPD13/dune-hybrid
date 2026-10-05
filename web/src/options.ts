// The options form, shared by the title screen and the in-game menu.

import { LANGUAGES, type Settings, loadSettings, saveSettings } from "./settings";

/** Fill `form` from the template and keep settings in sync with it. */
export function mountOptions(form: HTMLElement, onChange: (s: Settings) => void) {
  const tpl = document.getElementById("options-template") as HTMLTemplateElement;
  form.replaceChildren(tpl.content.cloneNode(true));
  const lang = form.querySelector<HTMLSelectElement>('select[name="language"]')!;
  for (const [code, name] of Object.entries(LANGUAGES)) lang.add(new Option(name, code));

  const field = (name: string) => form.querySelector<HTMLInputElement | HTMLSelectElement>(`[name="${name}"]`)!;
  const sync = () => {
    const s = loadSettings();
    for (const [k, v] of Object.entries(s)) {
      const el = form.querySelector<HTMLInputElement | HTMLSelectElement>(`[name="${k}"]`);
      if (!el) continue;
      if (el instanceof HTMLInputElement && el.type === "checkbox") el.checked = Boolean(v);
      else el.value = String(v);
    }
  };
  form.addEventListener("input", () => {
    const s: Settings = {
      language: field("language").value as Settings["language"],
      music: Number(field("music").value),
      voices: Number(field("voices").value),
      display: field("display").value as Settings["display"],
      touchMode: field("touchMode").value as Settings["touchMode"],
      buttons: field("buttons").value as Settings["buttons"],
      batterySaver: (field("batterySaver") as HTMLInputElement).checked,
    };
    saveSettings(s);
    onChange(s);
  });
  sync();
  return { sync };
}
