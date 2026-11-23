// Browser preview only: synthetic sessions for the Analytics page (no real names or IDs).
import type { Analysis, SessionSummary } from "./types";

/** Small deterministic random generator so screenshots stay the same. */
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

const WEAPONS = ["Weapon 1", "Weapon 2", "Weapon 3", "Sidearm", "Knife", "Rocket launcher"];

export function demoAnalyses(n = 8): Analysis[] {
  const now = Math.floor(Date.now() / 1000);
  return Array.from({ length: n }, (_, i) => {
    const r = rng(i + 7);
    const started = now - (n - i) * 86400 - 3600;
    const minutes = 35 + Math.floor(r() * 50);
    const kills = 15 + Math.floor(r() * 40);
    const deaths = 8 + Math.floor(r() * 25);
    const hs = Math.floor(kills * (0.15 + r() * 0.2));
    const xp = 6000 + Math.floor(r() * 30000);
    const series = Array.from({ length: minutes + 1 }, (_, m) => {
      const k = Math.round((kills * m) / minutes);
      const d = Math.round((deaths * m) / minutes + Math.sin(m / 4 + i) * 1.2);
      const h = Math.round((hs * m) / minutes);
      return { minute: m, kills: k, deaths: Math.max(0, d), headshots: h, xp: Math.round((xp * m) / minutes), kd: k / Math.max(1, d), hsr: k ? h / k : 0 };
    });
    const weapons = WEAPONS.map((name, w) => {
      const wk = Math.max(0, Math.round(kills * [0.45, 0.25, 0.12, 0.08, 0.05, 0.05][w]! * (0.7 + r() * 0.6)));
      const wh = Math.round(wk * (0.1 + r() * 0.3));
      return { id: w + 1, name, kills: wk, headshots: wh, hsr: wk ? wh / wk : 0, deaths_by: Math.floor(r() * 4) };
    });
    const vs = Math.floor(kills * (0.3 + r() * 0.3));
    return {
      id: i + 1,
      character_id: "1",
      character_name: i % 3 === 2 ? "DemoMedicNC" : "DemoSoldierTR",
      faction_id: i % 3 === 2 ? 2 : 3,
      started_at: started,
      ended_at: started + minutes * 60,
      data_version: 2,
      stats: {
        kills, deaths, headshots: hs, teamkills: 0, teamkilled: 0, suicides: Math.floor(r() * 2), revives: Math.floor(r() * 20), revived: Math.floor(r() * 6),
        revived_deaths: Math.floor(r() * 5), assists: Math.floor(r() * 15), xp, vehicle_kills: Math.floor(r() * 4), vehicles_lost: Math.floor(r() * 2),
        facility_captures: Math.floor(r() * 3), facility_defends: Math.floor(r() * 3),
      },
      rates: { duration_secs: minutes * 60, kd: kills / deaths, kd_revived: kills / Math.max(1, deaths - 3), kpm: kills / minutes, hsr: hs / kills, xp_per_hour: (xp / minutes) * 60 },
      best_streak: 3 + Math.floor(r() * 9),
      weapons,
      vehicles: [{ id: 1, name: "Vehicle", kills: 2 }],
      kills: [],
      deaths: [],
      support: [],
      achievements: [],
      series,
      teams: [
        { team_id: 1, kills: vs, deaths: Math.floor(deaths * 0.5) },
        { team_id: i % 3 === 2 ? 3 : 2, kills: kills - vs - 2, deaths: Math.floor(deaths * 0.4) },
        { team_id: 4, kills: 2, deaths: Math.ceil(deaths * 0.1) },
      ],
      classes: {
        enemy: [
          { class: "heavy_assault", kills: Math.round(kills * 0.34), deaths: Math.round(deaths * 0.4) },
          { class: "medic", kills: Math.round(kills * 0.26), deaths: Math.round(deaths * 0.2) },
          { class: "infiltrator", kills: Math.round(kills * 0.14), deaths: Math.round(deaths * 0.2) },
          { class: "light_assault", kills: Math.round(kills * 0.12), deaths: Math.round(deaths * 0.1) },
          { class: "engineer", kills: Math.round(kills * 0.1), deaths: Math.round(deaths * 0.05) },
          { class: "max", kills: Math.round(kills * 0.04), deaths: Math.round(deaths * 0.05) },
        ],
        own: [{ class: "heavy_assault", kills, deaths, secs: minutes * 60 }],
      },
    } satisfies Analysis;
  });
}

export function demoSummaries(list: Analysis[]): SessionSummary[] {
  return list
    .map((a) => ({
      id: a.id ?? 0,
      character_id: a.character_id,
      character_name: a.character_name,
      started_at: a.started_at,
      ended_at: a.ended_at,
      kills: a.stats.kills,
      deaths: a.stats.deaths,
      headshots: a.stats.headshots,
      xp: a.stats.xp,
      data_version: 2,
      faction_id: a.faction_id,
    }))
    .reverse();
}
