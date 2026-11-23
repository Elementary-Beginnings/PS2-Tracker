// Draws a crosshair PNG pixel-exactly (nearest neighbour) onto a canvas.
// Canvas pixels map 1:1 to physical screen pixels:
// backing store = image × scale, CSS size = backing store / devicePixelRatio.

export interface DrawOptions {
  scale: number;
  opacity: number;
  tint: string;
  /**
   * "multiply" (default, the crosshair tint: dark outlines stay dark, colours
   * mix) or "recolor" (hit feedback: every pixel takes the colour at its own
   * brightness, so a green crosshair really turns red).
   */
  tintMode?: "multiply" | "recolor";
  /** extra magnification for the preview only (CSS) */
  zoom?: number;
  /** canvas size in device pixels (window size); the image is centred inside */
  frame?: { width: number; height: number };
}

/**
 * Strength of a hit-feedback effect (0–1) after `t` ms: full for the first
 * 30 % (at least 30 ms), then fading out linearly (or off at once without fade).
 */
export function fxAlpha(t: number, duration: number, fade: boolean): number {
  if (t < 0 || t >= duration) return 0;
  const hold = Math.min(duration, Math.max(30, duration * 0.3));
  if (!fade || t < hold) return 1;
  return Math.max(0, 1 - (t - hold) / Math.max(1, duration - hold));
}

/** Colours RGBA pixels in place: colour × the pixel's brightness (max channel), alpha unchanged. */
export function recolor(d: Uint8ClampedArray, r: number, g: number, b: number) {
  for (let i = 0; i < d.length; i += 4) {
    const v = Math.max(d[i] ?? 0, d[i + 1] ?? 0, d[i + 2] ?? 0) / 255;
    d[i] = r * v;
    d[i + 1] = g * v;
    d[i + 2] = b * v;
  }
}

export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("image not readable"));
    img.src = src;
  });
}

export function draw(canvas: HTMLCanvasElement, img: HTMLImageElement, o: DrawOptions, dpr = window.devicePixelRatio || 1) {
  const scale = Math.max(1, Math.round(o.scale));
  const iw = img.naturalWidth * scale;
  const ih = img.naturalHeight * scale;
  const w = Math.max(iw, o.frame?.width ?? iw);
  const h = Math.max(ih, o.frame?.height ?? ih);
  // integer offset: window and image size have the same parity (see Rust)
  const ox = Math.floor((w - iw) / 2);
  const oy = Math.floor((h - ih) / 2);
  canvas.width = w;
  canvas.height = h;
  const zoom = o.zoom ?? 1;
  canvas.style.width = `${(w * zoom) / dpr}px`;
  canvas.style.height = `${(h * zoom) / dpr}px`;
  canvas.style.imageRendering = "pixelated";
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = 1;
  ctx.globalCompositeOperation = "source-over";
  ctx.drawImage(img, ox, oy, iw, ih);
  if (/^#[0-9a-fA-F]{6}$/.test(o.tint) && o.tintMode === "recolor") {
    const n = parseInt(o.tint.slice(1), 16);
    const px = ctx.getImageData(ox, oy, iw, ih);
    recolor(px.data, (n >> 16) & 255, (n >> 8) & 255, n & 255);
    ctx.putImageData(px, ox, oy);
  } else if (/^#[0-9a-fA-F]{6}$/.test(o.tint)) {
    // tint: colour only bright pixels, dark outlines stay dark (multiply)
    ctx.globalCompositeOperation = "multiply";
    ctx.fillStyle = o.tint;
    ctx.fillRect(ox, oy, iw, ih);
    ctx.globalCompositeOperation = "destination-in";
    ctx.drawImage(img, ox, oy, iw, ih);
    ctx.globalCompositeOperation = "source-over";
  }
  canvas.style.opacity = String(Math.min(1, Math.max(0, o.opacity)));
}
