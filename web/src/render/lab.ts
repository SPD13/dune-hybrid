// Dev only: the current frame through every preset, side by side.
//   __lab()                  all presets
//   __lab(["original","pixel"], 2)   chosen presets, columns
//   __lab(presets, 3, {}, { x: 200, y: 60, w: 80 })   zoom on a 4:3 crop
//     (x, y in game pixels; w = crop width in game pixels)

import { DEFAULTS, type Graphics, PRESETS, type Preset } from "../settings";
import { GlRenderer } from "./gl";

export function openLab(
  frame: Uint8Array,
  presets = Object.keys(PRESETS) as Preset[],
  columns = 3,
  extra: Partial<Graphics> = {},
  crop?: { x: number; y: number; w: number },
) {
  document.getElementById("lab")?.remove();
  const lab = document.createElement("div");
  lab.id = "lab";
  lab.style.cssText = `position:fixed;inset:0;z-index:50;background:#111;display:grid;grid-template-columns:repeat(${columns},1fr);gap:4px;padding:4px;overflow:auto`;
  lab.onclick = (e) => e.target === lab && lab.remove();
  document.body.append(lab);
  const cellW = Math.floor((innerWidth - 8 - 4 * (columns - 1)) / columns);
  for (const name of presets) {
    const cell = document.createElement("figure");
    cell.style.cssText = "margin:0;color:#ccc;font:12px system-ui";
    const canvas = document.createElement("canvas");
    const zoom = crop ? 320 / crop.w : 1;
    const w = Math.round(cellW * zoom);
    const h = Math.round((w * 3) / 4);
    canvas.style.cssText = `width:${w}px;height:${h}px;display:block`;
    if (crop) canvas.style.margin = `${(-crop.y / 200) * h}px 0 0 ${(-crop.x / 320) * w}px`;
    const view = document.createElement("div");
    view.style.cssText = `width:${cellW}px;height:${Math.round((cellW * 3) / 4)}px;overflow:hidden`;
    view.append(canvas);
    const cap = document.createElement("figcaption");
    cap.textContent = name;
    cell.append(view, cap);
    lab.append(cell);
    const r = GlRenderer.create(canvas, false);
    if (!r) continue;
    r.resize(w, h, devicePixelRatio);
    r.setGraphics({ ...DEFAULTS.graphics, ...(name === "custom" ? {} : PRESETS[name as Exclude<Preset, "custom">]), ...extra });
    r.frame(frame);
  }
}
