// Display formatting (English: point as decimal separator, 24-hour clock).
import { LOCALE, t } from "./i18n";
import type { FeedRow } from "./types";

const nf = (digits: number) =>
  new Intl.NumberFormat(LOCALE, { minimumFractionDigits: digits, maximumFractionDigits: digits });

export const num = (v: number, digits = 0): string => nf(digits).format(Number.isFinite(v) ? v : 0);

export const pct = (v: number, digits = 1): string => `${num(v * 100, digits)} %`;

const pad = (n: number) => String(n).padStart(2, "0");

export function duration(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return h > 0 ? `${h}:${pad(m)}:${pad(r)}` : `${m}:${pad(r)}`;
}

/** "2026-11-20 21:37" in local time. */
export function dateTime(unix: number): string {
  const d = new Date(unix * 1000);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** "21:37:05" in local time. */
export function clock(unix: number): string {
  const d = new Date(unix * 1000);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** Difference from the lifetime value as "+12 %" / "−5 %". */
export function delta(p: number | null): string {
  if (p === null || !Number.isFinite(p)) return "–";
  const sign = p > 0.05 ? "+" : p < -0.05 ? "−" : "±";
  return `${sign}${num(Math.abs(p), 0)} %`;
}

const FACTION_STYLE: Record<number, { short: string; color: string }> = {
  1: { short: "VS", color: "#8a4fd8" },
  2: { short: "NC", color: "#2f7fd7" },
  3: { short: "TR", color: "#d0393e" },
  4: { short: "NSO", color: "#8c939d" },
};

export function faction(id: number | null | undefined) {
  const s = FACTION_STYLE[id ?? 0];
  if (!s) return { name: t.factions.unknown, short: "?", color: "#8c939d" };
  return { ...s, name: t.factions[id as 1 | 2 | 3 | 4] };
}

const WORLDS: Record<number, string> = {
  1: "Osprey (Connery)",
  10: "Wainwright (Miller)",
  13: "Cobalt",
  17: "Emerald",
  19: "Jaeger",
  40: "SolTech",
  1000: "Genudine",
  2000: "Ceres",
};

export const worldName = (id: number | null | undefined): string =>
  id == null ? "–" : (WORLDS[id] ?? `Server ${id}`);

/** Fallback for the continents (zone definition IDs) until Census answered. */
const ZONES: Record<number, string> = { 2: "Indar", 4: "Hossin", 6: "Amerish", 8: "Esamir", 14: "Koltyr", 344: "Oshur" };

/** "Server / Continent" of a running session: Census names, else the known
 *  lists; unknown parts are left out ("" if neither is known). */
export function locationLabel(l: { world_id?: number | null; zone_id?: number | null; world_name?: string | null; zone_name?: string | null }): string {
  const w = l.world_name ?? (l.world_id != null ? worldName(l.world_id) : null);
  const z = l.zone_name ?? (l.zone_id != null ? (ZONES[l.zone_id] ?? null) : null);
  return [w, z].filter(Boolean).join(" / ");
}

export const feedLabel = (kind: string): string => t.feed[kind] ?? kind;

/** "12 s ago", "5 min ago", "3 h ago", else the date. */
export function ago(ts: number, now = Date.now() / 1000): string {
  const d = Math.max(0, Math.round(now - ts));
  if (d < 60) return `${d} s ago`;
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  return new Date(ts * 1000).toLocaleDateString(LOCALE);
}

/** Who the feed row is about: the other player, else the vehicle, else why there is no name. */
export const feedWho = (e: FeedRow): string =>
  e.other_name || e.vehicle_name || t.feedWho[e.other_label || "unknown"] || t.feedWho.unknown || "";

/**
 * Colour class of a feed row: kill (green), death (orange), teamkill (yellow).
 * Symbol and label carry the same information, colour is never the only cue.
 */
export const feedTone = (kind: string): "kill" | "death" | "tk" =>
  kind === "kill" || kind === "vehicle_kill" ? "kill" : kind === "teamkill" ? "tk" : "death";
/** Same symbols in the app, the overlay and the OBS source (colour is never the only cue). */
export const feedIcon: Record<string, string> = {
  kill: "✕",
  death: "☠",
  vehicle_kill: "⛟",
  vehicle_lost: "⛟",
  teamkill: "!",
  teamkilled: "!",
  suicide: "↺",
};
