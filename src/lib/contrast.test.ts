// Contrast check (WCAG 2.x) for the kill-feed colours in the app and the overlay.
// @ts-expect-error -- runs under Node in vitest; @types/node is not a dependency
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (rel: string): string => readFileSync(new URL(rel, import.meta.url), "utf8") as string;
const css = read("./app.css");
const overlay = read("../overlay/Overlay.svelte");

const lum = (hex: string): number => {
  const n = parseInt(hex.slice(1), 16);
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
};
const contrast = (a: string, b: string): number => {
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};

/** Values of `--name: #rrggbb` in the order they appear. */
const vars = (css: string, name: string): string[] =>
  [...css.matchAll(new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`, "g"))].map((m) => m[1] ?? "");
/** The i-th value of a variable; fails the test when it is missing. */
const v = (css: string, name: string, i = 0): string => {
  const x = vars(css, name)[i];
  if (!x) throw new Error(`--${name} #${i} missing`);
  return x;
};

const FEED = ["kill", "death", "tk", "hs", "hs-taken"];

describe("kill-feed colours", () => {
  it("reach 4.5:1 on the panel in dark and light theme", () => {
    const [panelDark, panelLight] = [v(css, "panel"), v(css, "panel", 1)];
    for (const name of FEED) {
      const [dark, light, system] = [v(css, name), v(css, name, 1), v(css, name, 2)];
      expect(contrast(dark, panelDark), `${name} dark`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(light, panelLight), `${name} light`).toBeGreaterThanOrEqual(4.5);
      expect(system, `${name} system-light equals light`).toBe(light);
    }
  });
  it("overlay uses the dark values and stays readable on its background", () => {
    for (const name of FEED) {
      const dark = v(css, name);
      expect(v(overlay, name), name).toBe(dark);
      // overlay card background rgb(15 18 24); worst case is a bright game
      // scene behind a thin card, covered by the text contour (text-shadow)
      expect(contrast(dark, "#0f1218"), name).toBeGreaterThanOrEqual(7);
    }
  });
  it("kill, death and both headshot colours are distinguishable", () => {
    const [kill, death, hs, taken] = [v(css, "kill"), v(css, "death"), v(css, "hs"), v(css, "hs-taken")];
    const pairs: [string, string][] = [
      [kill, death],
      [hs, taken],
      [hs, kill],
      [taken, death],
    ];
    for (const [a, b] of pairs) expect(a).not.toBe(b);
  });
});
