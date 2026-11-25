// Sample data for the browser preview only (`pnpm dev` without Tauri). Never used in the app.
import type { Config, EssStatus, LastSession, MoveLayerState, Snapshot, VoiceOverview } from "./types";

export function demoCrosshairs() {
  const c = document.createElement("canvas");
  c.width = c.height = 23;
  const g = c.getContext("2d")!;
  g.fillStyle = "#50ff78";
  g.fillRect(11, 2, 1, 7);
  g.fillRect(11, 14, 1, 7);
  g.fillRect(2, 11, 7, 1);
  g.fillRect(14, 11, 7, 1);
  g.fillRect(11, 11, 1, 1);
  return [{ file: "kreuz-punkt-gruen.png", width: 23, height: 23, data_url: c.toDataURL("image/png") }];
}

export function demoOverview(): VoiceOverview {
  const meta = (name: string, author: string, source = "") => ({ name, author, version: "1.0.0", license: "", description: "", source });
  const now = Math.floor(Date.now() / 1000);
  const ev = (key: string, label: string, trigger: string | null, recursion = false, on = "kill", last: number | null = null) => {
    const rules = trigger ? [{ event: key, on, description: trigger }] : [];
    return { key, label, trigger, priority: 50, recursion, rules, default_rules: rules, last_played: last };
  };
  return {
    packs: [
      { id: "standard-tones", meta: meta("Standard tones", "PS2 Session Tracker"), format: "open", events: 16, files: 16, active: true, position: 1, warnings: [], background: null, sounds: ["kill", "headshot", "multikill", "streak", "death", "revive"] },
      { id: "sample-voice", meta: meta("Sample Voice Pack", "Someone", "recursion: SharpSerializer (binary)"), format: "open", events: 96, files: 212, active: true, position: 0, warnings: [], background: null, sounds: ["HEAD_SHOT", "DOUBLE_KILL", "TRIPLE_KILL", "PER_KILL", "KILLING_SPREE"] },
      { id: "classic-pack", meta: meta("Classic Pack", "Another author"), format: "recursion", events: 24, files: 24, active: false, position: null, warnings: ["REVENGE: file missing"], background: null, sounds: [] },
    ],
    invalid: [],
    events: [
      ev("kill", "Kill", "Kill without headshot", false, "kill", now - 40),
      ev("headshot", "Headshot", "Kill with headshot", false, "kill", now - 20),
      ev("multikill", "Multikill", "Kill chain (>= 2 kills within the multikill window)", false, "multikill"),
      ev("streak", "Kill streak", "Kill streak milestone", false, "streak"),
      ev("death", "Death", "Death by an enemy", false, "death", now - 45),
      ev("teamkill", "Teamkill", "You killed an ally", false, "teamkill"),
      ev("teamkilled", "Teamkilled", "An ally killed you", false, "teamkilled"),
      ev("DEATH_INSULT", "Death insult", "Death by an enemy", true, "death"),
      ev("SUICIDE_BOMBER", "Suicide bomber", null, true),
      ev("VIP_KILL", "VIP kill", null, true),
      ev("revive", "Revive", "You revived someone"),
      ev("vehicle_kill", "Vehicle destroyed", "Vehicle destroyed"),
      ev("session_start", "Session start", "Session started"),
      ev("HEAD_SHOT", "Head shot", "Kill with headshot", true, "kill", now - 20),
      ev("DOUBLE_KILL", "Double kill", "Kill chain of 2", true),
      ev("TRIPLE_KILL", "Triple kill", "Kill chain of 3", true),
      ev("KILLING_SPREE", "Killing spree", "Kill streak of 5", true),
      ev("PER_KILL", "Per kill", "Every kill", true),
      ev("MASTER_MEDIC", "Master medic", null, true),
    ],
    triggers_file: "~/.config/ps2-tracker/triggers.toml",
  };
}

// URL parameters of the preview: faction=1|2|3|4, theme=dark|light, page=…,
// empty=1 (no session; with last=old the last session is too old to resume),
// paused=1 (logged out, waiting for a relogin), wizard=1. Character names are
// fictitious.
// clearly synthetic names (screenshots must not show real characters)
const NAMES: Record<number, string> = { 1: "DemoSoldierVS", 2: "DemoSoldierNC", 3: "DemoSoldierTR", 4: "DemoSoldierNSO" };
const facOf = (p?: URLSearchParams) => Number(p?.get("faction") ?? 3) || 3;

export function demoConfig(p?: URLSearchParams): Config {
  const f = facOf(p);
  return {
    // not a real ID; masked in the UI anyway
    census: { service_id: "demo0preview0id" },
    character: [{ name: NAMES[f] ?? "Sample", id: "1", faction_id: f, enabled: true }],
    squad: [],
    discord: { enabled: false, app_id: "", show_character: true, show_kd: true, show_kills: true, show_duration: true, show_location: false },
    updates: { check_on_start: false },
    online_sync: { enabled: false, api_base: "https://stats.elme.games/api/v1", discord_client_id: "" },
    session: { idle_timeout_minutes: 30, resume_minutes: 15, streak_step: 5, killfeed_len: 12 },
    overlay: {
      anchor: "top-right",
      show_kd_revived: false,
      show_squad: false,
      popup_monitor: "",
      squad_metric: "kd",
      squad_min_kills: 5,
      size: "auto",
      style: "card",
      popup_anchor: "top-center",
      popup_x: 0,
      popup_y: 160,
      popup_width: 420,
      popup_height: 100,
      popup_scale: 1,
      popup_duration_ms: 4000,
      popup_text: true,
      popup_style: "plain",
      popup_opacity: 1,
      enabled: true,
      monitor: "",
      x: 40,
      y: 200,
      width: 360,
      height: 520,
      background_opacity: 0.55,
      scale: 1,
      click_through: true,
      show_stats: true,
      show_killfeed: true,
      show_weapons: true,
      show_toasts: true,
      show_comparison: true,
      show_my_class: p?.has("myclass") ?? false,
      hotkey_edit: "Ctrl+F2",
      hotkey_finish_move: "Ctrl+Escape",
      move_idle_timeout_secs: 300,
      pack_background: true, compact: false, auto_hide: false, hotkey_toggle: "Ctrl+Shift+F10",
    },
    voice: {
      enabled: true,
      volume: 0.6,
      output_device: "",
      active_packs: ["standard-tones"],
      overrides: {},
      muted: ["kill"],
      max_concurrent: 1,
      multikill_window_secs: 4,
      kill_key: "",
      chain_style: "auto",
      builtin_fallback: false,
      trigger_overrides: {},
    },
    crosshair: {
      enabled: false,
      file: "kreuz-punkt-gruen.png",
      scale: 1,
      opacity: 1,
      tint: "",
      offset_x: 0,
      offset_y: 0,
      monitor: "",
      hotkey_toggle: "Ctrl+Shift+F11",
      hotkey_cycle: "Ctrl+Shift+F12",
      auto_hide: false,
      game_window: { class: "", title: "", process: "" },
      game_match: "planetside2|steam_app_218230|PlanetSide 2",
      hit_feedback: {
        enabled: true,
        kill: { enabled: true, mode: "tint", color: "#ffffff", file: "", duration_ms: 150, fade: true, peak: 1 },
        headshot: { enabled: true, mode: "tint", color: "#ff3b30", file: "", duration_ms: 150, fade: true, peak: 1 },
        assist: { enabled: false, mode: "tint", color: "#62b4ff", file: "", duration_ms: 150, fade: true, peak: 1 },
        vehicle_kill: { enabled: false, mode: "tint", color: "#ffd24a", file: "", duration_ms: 150, fade: true, peak: 1 },
        vehicle_damage: { enabled: false, mode: "tint", color: "#ffd24a", file: "", duration_ms: 80, fade: true, peak: 0.7 },
      },
    },
    obs: { enabled: false, port: 47823 },
    ui: {
      theme: (p?.get("theme") as "dark" | "light" | null) ?? "dark",
      start_minimized: false,
      x11_backend: true,
      primary_kd: "true",
      webkit_dmabuf_workaround: true,
      debug_console: false,
      accent: "faction",
      accent_custom: "#4fc3c9",
      setup_done: true,
    },
  };
}

export function demoStatus(_p?: URLSearchParams): EssStatus {
  return { state: "connected", detail: "Browser preview – sample data", retry_in_secs: null, last_event_at: null };
}

export function demoSnapshot(p?: URLSearchParams): Snapshot {
  const now = Math.floor(Date.now() / 1000);
  const f = facOf(p);
  return {
    session_id: null,
    character_id: "1",
    character_name: NAMES[f] ?? "Sample",
    faction_id: f,
    started_at: now - 2700,
    ended_at: null,
    active: true,
    stats: {
      kills: 42, deaths: 21, headshots: 11, teamkills: 1, teamkilled: 0, suicides: 1, revives: 3,
      revived: 2, revived_deaths: 2, assists: 9, xp: 18450, vehicle_kills: 2, vehicles_lost: 1, facility_captures: 1, facility_defends: 2,
    },
    rates: { duration_secs: 2700, kd: 2, kd_revived: 2.47, kpm: 0.93, hsr: 0.262, xp_per_hour: 24600 },
    streak: 4,
    best_streak: 9,
    killfeed: [
      { timestamp: now - 20, kind: "kill", other_id: "2", other_name: "Enemy A", weapon_name: "Weapon 1", headshot: true, vehicle_name: "", other_class: "medic", my_class: "heavy_assault" },
      { timestamp: now - 45, kind: "death", other_id: "3", other_name: "Enemy B", weapon_name: "Weapon 2", headshot: false, vehicle_name: "", other_class: "infiltrator", my_class: "heavy_assault" },
      { timestamp: now - 80, kind: "vehicle_kill", other_id: "4", other_name: "Enemy C", weapon_name: "Rocket launcher", headshot: false, vehicle_name: "Sunderer" },
      { timestamp: now - 100, kind: "death", other_id: "7", other_name: "Enemy F", weapon_name: "Weapon 3", headshot: true, vehicle_name: "", other_class: "max", my_class: "heavy_assault" },
      { timestamp: now - 130, kind: "kill", other_id: "5", other_name: "Enemy D", weapon_name: "Weapon 1", headshot: false, vehicle_name: "", other_class: "light_assault", my_class: "medic" },
      { timestamp: now - 150, kind: "death", other_id: "0", other_name: "", other_label: "environment", weapon_name: "", headshot: false, vehicle_name: "" },
      { timestamp: now - 160, kind: "teamkill", other_id: "6", other_name: "Friend E", weapon_name: "", headshot: false, vehicle_name: "", other_class: "engineer", my_class: "medic" },
    ],
    weapons: [
      { weapon_id: 1, name: "Weapon 1", kills: 30, headshots: 9, hsr: 0.3, deaths_by: 0, lifetime_kills: 4000, lifetime_hsr: 0.22 },
      { weapon_id: 2, name: "Weapon 2", kills: 12, headshots: 2, hsr: 0.17, deaths_by: 1, lifetime_kills: null, lifetime_hsr: null },
    ],
    vehicle_kills: [["Vehicle", 2]],
    achievements: [{ id: "streak_5", name: "Warmed Up", description: "5 kills without dying", timestamp: now - 600 }],
    comparison: {
      kd: { session: 2, lifetime: 1.1, delta_pct: 82 },
      kpm: { session: 0.93, lifetime: 0.45, delta_pct: 107 },
      hsr: { session: 0.262, lifetime: 0.18, delta_pct: 46 },
      xp_per_hour: { session: 24600, lifetime: 18000, delta_pct: 37 },
      lifetime_kills: 50000,
      lifetime_deaths: 45000,
    },
    world_id: 10,
    zone_id: 2,
    world_name: "Miller",
    zone_name: "Indar",
    class: "heavy_assault",
    classes: {
      enemy: [
        { class: "heavy_assault", kills: 14, deaths: 8 },
        { class: "medic", kills: 11, deaths: 4 },
        { class: "infiltrator", kills: 6, deaths: 5 },
        { class: "light_assault", kills: 5, deaths: 2 },
        { class: "engineer", kills: 4, deaths: 1 },
        { class: "max", kills: 2, deaths: 1 },
      ],
      own: [
        { class: "heavy_assault", kills: 29, deaths: 15, secs: 1620 },
        { class: "medic", kills: 13, deaths: 6, secs: 780 },
      ],
    },
    paused_at: p?.has("paused") ? now - 135 : null,
    resume_until: p?.has("paused") ? now - 135 + 15 * 60 : null,
    paused_secs: 0,
    squad_top: [
      { character_id: "11", name: "DemoMedic", faction_id: f, own: false, kills: 31, deaths: 9, value: 3.44, class: "medic" },
      { character_id: "1", name: NAMES[f] ?? "Sample", faction_id: f, own: true, kills: 42, deaths: 21, value: 2, class: "heavy_assault" },
      { character_id: "12", name: "DemoEngineer", faction_id: f, own: false, kills: 2, deaths: 1, value: 2, qualified: false, session: true, class: null },
    ],
  };
}

/** Last ended session for "Resume last session" in the preview (`?empty=1`):
 *  ended 40 minutes ago, or 9 hours ago with `last=old` (too old). */
export function demoLastSession(p?: URLSearchParams): LastSession {
  const now = Math.floor(Date.now() / 1000);
  const ended = p?.get("last") === "old" ? now - 9 * 3600 : now - 40 * 60;
  const until = ended + 6 * 3600;
  return { id: 1, started_at: ended - 5400, ended_at: ended, resumable: now <= until, resumable_until: until };
}

/** Move layer in the browser preview (screenshots): a 1920×1080 monitor, the
 *  stats overlay top right, the pop-up top centre; `?grab=1` shows the hint for
 *  a game that holds the cursor. */
export function demoMoveLayer(p?: URLSearchParams): MoveLayerState {
  const mon = { name: "DP-1", x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1, primary: true };
  return {
    monitor: mon,
    monitors: [mon, { name: "HDMI-A-1", x: 1920, y: 0, width: 2560, height: 1440, scale_factor: 1, primary: false }],
    // sizes as the app measures the demo content (overlay full preset, pop-up sample)
    stats: { monitor: "DP-1", area: [0, 0, 1920, 1080], anchor: "top-right", size: [394, 378], pos: [1502, 24] },
    popup: { monitor: "DP-1", area: [0, 0, 1920, 1080], anchor: "top-center", size: [352, 58], pos: [784, 160] },
    selected: "stats",
    focus: true,
    pointer_free: p?.has("grab") ? false : true,
    hotkey_edit: "Ctrl+F2",
    hotkey_finish: "Ctrl+Escape",
    failed_hotkeys: p?.has("taken") ? ["Ctrl+F2"] : [],
    idle_timeout_secs: 300,
  };
}
