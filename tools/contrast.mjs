#!/usr/bin/env node
// WCAG 2.1 contrast check of the accent colours (src/lib/theme.ts) against the
// surfaces they are used on. AA: 4.5:1 for text, 3:1 for UI components.
// Usage: node tools/contrast.mjs   (exit code 1 if a pair fails)
import { readFileSync } from "node:fs";

const src = readFileSync(new URL("../src/lib/theme.ts", import.meta.url), "utf8");
const block = (name) => {
  const m = src.match(new RegExp(`export const ${name}[^=]*=\\s*(\\{[\\s\\S]*?\\n\\});`));
  if (!m) throw new Error(`${name} not found in theme.ts`);
  return Function(`return (${m[1]})`)();
};
const ACCENTS = block("ACCENTS");
const SURFACES = block("SURFACES");

const lin = (c) => {
  c /= 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const lum = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
};
const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

let fail = 0;
const rows = [];
for (const mode of ["dark", "light"]) {
  const s = SURFACES[mode];
  const checks = [
    ["text on bg", s.text, s.bg, 4.5],
    ["muted on panel", s.muted, s.panel, 4.5],
    ["good on panel", s.good, s.panel, 4.5],
    ["bad on panel", s.bad, s.panel, 4.5],
  ];
  for (const [name, a] of Object.entries(ACCENTS[mode])) {
    checks.push([`${name}: accent text on panel`, a.accent, s.panel, 4.5]);
    checks.push([`${name}: accent text on bg-2 (sidebar)`, a.accent, s.bg2, 4.5]);
    checks.push([`${name}: button text on accent`, a.ink, a.accent, 4.5]);
    checks.push([`${name}: focus ring vs bg`, a.accent, s.bg, 3]);
  }
  for (const [label, fg, bg, min] of checks) {
    const r = ratio(fg, bg);
    const ok = r >= min;
    if (!ok) fail++;
    rows.push(`${ok ? "ok  " : "FAIL"} ${mode.padEnd(5)} ${label.padEnd(40)} ${r.toFixed(2).padStart(5)}:1 (min ${min})`);
  }
}
console.log(rows.join("\n"));
console.log(fail ? `\n${fail} pair(s) below WCAG 2.1 AA` : "\nall pairs meet WCAG 2.1 AA");
process.exit(fail ? 1 : 0);
