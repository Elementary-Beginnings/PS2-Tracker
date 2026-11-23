// Colour system: neutral by default, the faction of the active character when
// "Accent: Faction" is chosen. Values are checked by tools/contrast.mjs
// (WCAG 2.1 AA) – keep the object literals simple, the script parses them.

export type AccentName = "neutral" | "vs" | "nc" | "tr" | "nso";

export const ACCENTS: Record<"dark" | "light", Record<AccentName, { accent: string; ink: string }>> = {
  dark: {
    neutral: { accent: "#4fc3c9", ink: "#0b1014" },
    vs: { accent: "#b892ff", ink: "#0b1014" },
    nc: { accent: "#5ea8ff", ink: "#0b1014" },
    tr: { accent: "#ff7272", ink: "#0b1014" },
    nso: { accent: "#b9c0cb", ink: "#0b1014" },
  },
  light: {
    neutral: { accent: "#0b6f78", ink: "#ffffff" },
    vs: { accent: "#6a3dc2", ink: "#ffffff" },
    nc: { accent: "#1d5cb3", ink: "#ffffff" },
    tr: { accent: "#b0262c", ink: "#ffffff" },
    nso: { accent: "#545b66", ink: "#ffffff" },
  },
};

/** Surfaces as in app.css (for the contrast check). */
export const SURFACES = {
  dark: { bg: "#0f1218", bg2: "#151a22", panel: "#1a202a", text: "#e7e9ee", muted: "#9aa3b4", good: "#62b4ff", bad: "#ffa557" },
  light: { bg: "#f3f4f7", bg2: "#e9ecf1", panel: "#ffffff", text: "#1a1f29", muted: "#5a6372", good: "#1a5fb4", bad: "#a34a00" },
};

export function factionAccent(factionId: number | null | undefined): AccentName {
  return ({ 1: "vs", 2: "nc", 3: "tr", 4: "nso" } as Record<number, AccentName>)[factionId ?? 0] ?? "neutral";
}

/** Effective dark/light mode for a theme setting. */
export function effectiveMode(theme: string): "dark" | "light" {
  if (theme === "light" || theme === "dark") return theme;
  return typeof matchMedia !== "undefined" && matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

/** Sets --accent/--accent-ink on the root element. */
export function applyAccent(setting: string, custom: string, factionId: number | null | undefined, theme: string) {
  const mode = effectiveMode(theme);
  const root = document.documentElement;
  if (setting === "custom" && /^#[0-9a-f]{6}$/i.test(custom)) {
    root.style.setProperty("--accent", custom);
    root.style.setProperty("--accent-ink", inkFor(custom));
    root.dataset.accent = "custom";
    return;
  }
  const name: AccentName = setting === "faction" ? factionAccent(factionId) : "neutral";
  const a = ACCENTS[mode][name];
  root.style.setProperty("--accent", a.accent);
  root.style.setProperty("--accent-ink", a.ink);
  root.dataset.accent = name;
}

/** Black or white text, whichever contrasts more with the colour. */
export function inkFor(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const l = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  return (l + 0.05) / 0.05 > 1.05 / (l + 0.05) ? "#0b1014" : "#ffffff";
}
