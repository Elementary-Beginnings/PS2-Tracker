// The overlay background opacity must apply exactly once: one panel (the card)
// carries it, the sections inside have no background of their own.
// @ts-expect-error -- runs under Node in vitest; @types/node is not a dependency
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const overlay = readFileSync(new URL("../overlay/Overlay.svelte", import.meta.url), "utf8") as string;
const style = overlay.slice(overlay.indexOf("<style>"));

/** [selector, body] of every plain CSS rule (keyframes left out). */
const rules = (css: string): [string, string][] =>
  [...css.replace(/@keyframes[^{]*\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}/g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => [
    (m[1] ?? "").replace(/\/\*[\s\S]*?\*\//g, "").trim(),
    m[2] ?? "",
  ]);

describe("overlay background layers", () => {
  const all = rules(style);

  it("uses the opacity setting only on the card and the OBS toast", () => {
    const withAlpha = all.filter(([, body]) => /background[^;]*var\(--bgA\)/.test(body)).map(([sel]) => sel);
    expect(withAlpha.length).toBeGreaterThan(0);
    for (const sel of withAlpha) expect(sel).toMatch(/(^|\s)\.card$|^\.toast$/);
  });

  it("gives the sections inside the card no background", () => {
    const sections = /\.(head|tiles|tile|cmp|feed|fe|squad|sq|sq-head|weapons|wr)\b(?![-\w])/;
    for (const [sel, body] of all) {
      if (!sections.test(sel)) continue;
      // separator lines and text colours are fine, a fill is not
      expect(body, sel).not.toMatch(/(^|[;\s])background(-color|-image)?\s*:/);
    }
  });

  it("blurs the backdrop only in the OBS source", () => {
    const blur = all.filter(([, body]) => body.includes("backdrop-filter")).map(([sel]) => sel);
    expect(blur).toEqual([".obs .card"]);
    expect(overlay).toContain("class:obs");
  });
});
