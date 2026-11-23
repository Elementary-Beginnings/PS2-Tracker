// Crosshair window: draws the selected PNG pixel-exactly. No interaction (click-through).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { draw, fxAlpha, loadImage } from "../lib/crosshair";
import type { CrosshairFx, CrosshairRender } from "../lib/types";
import "./crosshair.css";

const canvas = document.getElementById("xh") as HTMLCanvasElement;
let last: CrosshairRender | null = null;
let fxRun = 0;

async function render(r: CrosshairRender | null) {
  last = r;
  fxRun++; // a running effect stops, the normal crosshair is drawn
  if (!r) {
    canvas.width = 0;
    return;
  }
  const img = await loadImage(r.data_url);
  const dpr = window.devicePixelRatio || 1;
  const frame = { width: Math.round(window.innerWidth * dpr), height: Math.round(window.innerHeight * dpr) };
  draw(canvas, img, { scale: r.scale, opacity: r.opacity, tint: r.tint, frame }, dpr);
}

/** Renders an image like the crosshair into an offscreen canvas of its own size. */
function layer(img: HTMLImageElement, scale: number, tint: string, tintMode: "multiply" | "recolor" = "multiply"): HTMLCanvasElement {
  const c = document.createElement("canvas");
  draw(c, img, { scale, opacity: 1, tint, tintMode }, 1);
  return c;
}

/**
 * Hit feedback: only the content of the existing canvas is redrawn – same
 * size, same position, no window change. A swapped image larger than the
 * crosshair window is clipped.
 */
async function fx(p: CrosshairFx) {
  const r = last;
  if (!r || canvas.width === 0) return;
  const run = ++fxRun;
  const e = p.effect;
  const base = await loadImage(r.data_url);
  const fxImg = e.mode === "swap" && p.data_url ? await loadImage(p.data_url) : base;
  const normal = layer(base, r.scale, r.tint);
  const flash = e.mode === "tint" ? layer(fxImg, r.scale, e.color, "recolor") : layer(fxImg, r.scale, r.tint);
  const ctx = canvas.getContext("2d");
  if (!ctx || run !== fxRun) return;
  const { width: W, height: H } = canvas;
  const at = (c: HTMLCanvasElement) => [Math.floor((W - c.width) / 2), Math.floor((H - c.height) / 2)] as const;
  canvas.style.opacity = "1";
  ctx.imageSmoothingEnabled = false;
  const start = performance.now();
  const frame = (now: number) => {
    if (run !== fxRun) return;
    const a = fxAlpha(now - start, e.duration_ms, e.fade);
    if (a <= 0) {
      void render(r);
      return;
    }
    ctx.clearRect(0, 0, W, H);
    ctx.globalAlpha = r.opacity * (1 - a);
    ctx.drawImage(normal, ...at(normal));
    ctx.globalAlpha = e.peak * a;
    ctx.drawImage(flash, ...at(flash));
    ctx.globalAlpha = 1;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

listen<CrosshairRender>("crosshair-render", (e) => render(e.payload));
listen<CrosshairFx>("crosshair-fx", (e) => fx(e.payload));
invoke<CrosshairRender | null>("crosshair_render_info").then(render);
// redraw when the scale changes (monitor/fractional scaling)
matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`).addEventListener("change", () => render(last));
window.addEventListener("resize", () => render(last));
