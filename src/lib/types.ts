// Mirror of the Rust types (serde). Character IDs are strings (larger than MAX_SAFE_INTEGER).

export interface CharacterSettings {
  name: string;
  id?: string;
  faction_id?: number;
  enabled: boolean;
}

export interface PackBackground {
  pack: string;
  data_url: string;
  width: number;
  height: number;
}

export interface WindowEntry {
  class: string;
  title: string;
  process: string;
  matches: boolean;
}

export interface GameStatus {
  state: "active" | "background" | "not_running" | "unknown";
  title: string;
}

export interface OverlaySettings {
  anchor: string;
  size: string;
  style: string;
  popup_anchor: string;
  popup_x: number;
  popup_y: number;
  popup_width: number;
  popup_height: number;
  popup_scale: number;
  popup_duration_ms: number;
  popup_text: boolean;
  popup_style: string;
  popup_opacity: number;
  auto_hide: boolean;
  pack_background: boolean;
  compact: boolean;
  show_kd_revived: boolean;
  show_squad: boolean;
  popup_monitor: string;
  squad_metric: string;
  squad_min_kills: number;
  enabled: boolean;
  monitor: string;
  x: number;
  y: number;
  width: number;
  height: number;
  background_opacity: number;
  scale: number;
  click_through: boolean;
  show_stats: boolean;
  show_killfeed: boolean;
  show_weapons: boolean;
  show_toasts: boolean;
  show_comparison: boolean;
  show_my_class: boolean;
  hotkey_edit: string;
  hotkey_finish_move: string;
  move_idle_timeout_secs: number;
  hotkey_toggle: string;
}

export interface VoiceSettings {
  enabled: boolean;
  volume: number;
  output_device: string;
  active_packs: string[];
  overrides: Record<string, string[]>;
  muted: string[];
  max_concurrent: number;
  multikill_window_secs: number;
  kill_key: string;
  chain_style: string;
  builtin_fallback: boolean;
  trigger_overrides: Record<string, TriggerRule[]>;
}

export interface CrosshairSettings {
  enabled: boolean;
  file: string;
  scale: number;
  opacity: number;
  tint: string;
  offset_x: number;
  offset_y: number;
  monitor: string;
  hotkey_toggle: string;
  hotkey_cycle: string;
  auto_hide: boolean;
  game_window: { class: string; title: string; process: string };
  game_match: string;
  hit_feedback: HitFeedback;
}

export interface Config {
  /** Own service ID; empty = built-in default (official builds) or "example". */
  census: { service_id: string };
  character: CharacterSettings[];
  squad: CharacterSettings[];
  discord: { enabled: boolean; app_id: string; show_character: boolean; show_kd: boolean; show_kills: boolean; show_duration: boolean; show_location: boolean };
  updates: { check_on_start: boolean };
  session: { idle_timeout_minutes: number; streak_step: number; killfeed_len: number };
  overlay: OverlaySettings;
  voice: VoiceSettings;
  crosshair: CrosshairSettings;
  obs: { enabled: boolean; port: number };
  ui: {
    theme: "system" | "dark" | "light";
    start_minimized: boolean;
    x11_backend: boolean;
    primary_kd: string;
    webkit_dmabuf_workaround: boolean;
    debug_console: boolean;
    accent: "faction" | "neutral" | "custom";
    accent_custom: string;
    setup_done: boolean;
  };
}

export interface SessionStats {
  kills: number;
  deaths: number;
  headshots: number;
  teamkills: number;
  teamkilled: number;
  suicides: number;
  revives: number;
  revived: number;
  revived_deaths?: number;
  assists: number;
  xp: number;
  vehicle_kills: number;
  vehicles_lost: number;
  facility_captures: number;
  facility_defends: number;
}

export interface Rates {
  duration_secs: number;
  kd: number;
  kd_revived: number;
  kpm: number;
  hsr: number;
  xp_per_hour: number;
}

export interface FeedRow {
  timestamp: number;
  kind: "kill" | "death" | "teamkill" | "teamkilled" | "suicide" | "vehicle_kill" | "vehicle_lost";
  other_id: string;
  other_name: string;
  /** "" | "self" | "environment" | "pending" | "unknown" */
  other_label?: string;
  weapon_name: string;
  headshot: boolean;
  vehicle_name: string;
  /** Class of the other party (kill: victim, death: attacker), null if unknown. */
  other_class?: ClassId | null;
  /** Own class at the time, null if unknown. */
  my_class?: ClassId | null;
}

/** Infantry class (from the ESS loadout ID, see crates/ps2-tracker-core/src/classes.rs). */
export type ClassId = "infiltrator" | "light_assault" | "medic" | "engineer" | "heavy_assault" | "max";

export interface ClassStats {
  enemy: { class: ClassId | null; kills: number; deaths: number }[];
  own: { class: ClassId | null; kills: number; deaths: number; secs: number }[];
}

export interface WeaponRow {
  weapon_id: number;
  name: string;
  kills: number;
  headshots: number;
  hsr: number;
  deaths_by: number;
  lifetime_kills: number | null;
  lifetime_hsr: number | null;
}

export interface Metric {
  session: number;
  lifetime: number;
  delta_pct: number | null;
}

export interface Comparison {
  kd: Metric;
  kpm: Metric;
  hsr: Metric;
  xp_per_hour: Metric;
  lifetime_kills: number;
  lifetime_deaths: number;
}

export interface Unlocked {
  id: string;
  name: string;
  description: string;
  timestamp: number;
}

export interface Snapshot {
  session_id: number | null;
  character_id: string;
  character_name: string;
  faction_id: number | null;
  started_at: number;
  ended_at: number | null;
  active: boolean;
  stats: SessionStats;
  rates: Rates;
  streak: number;
  best_streak: number;
  killfeed: FeedRow[];
  weapons: WeaponRow[];
  vehicle_kills: [string, number][];
  achievements: Unlocked[];
  comparison: Comparison | null;
  /** Overlay block "Squad top 3" (empty unless switched on). */
  squad_top?: SquadTopRow[];
  /** Last server/continent of the running session; names from Census (null while unknown). */
  world_id?: number | null;
  zone_id?: number | null;
  world_name?: string | null;
  zone_name?: string | null;
  /** Current own class (last loadout seen). */
  class?: ClassId | null;
  /** Kills/deaths by enemy class and own class use (data version 4). */
  classes?: ClassStats;
}

/** One object in the move layer (physical pixels, absolute). */
export interface MovedObject {
  monitor: string;
  area: [number, number, number, number];
  anchor: string;
  size: [number, number];
  pos: [number, number];
}

export interface MoveLayerState {
  monitor: MonitorInfo;
  monitors: MonitorInfo[];
  stats: MovedObject;
  popup: MovedObject;
  selected: "stats" | "popup";
  /** null = unknown (Wayland, Windows) */
  focus: boolean | null;
  pointer_free: boolean | null;
  hotkey_edit: string;
  hotkey_finish: string;
  failed_hotkeys: string[];
  idle_timeout_secs: number;
}

export interface HotkeyFailure {
  hotkey: string;
  action: string;
  reason: string;
}

export interface SquadTopRow {
  character_id: string;
  name: string;
  faction_id: number | null;
  own: boolean;
  kills: number;
  deaths: number;
  value: number;
  /** Enough kills for K/D/KPM; otherwise shown dimmed after the ranked ones. */
  qualified?: boolean;
  /** false: added, but no event yet. */
  session?: boolean;
  /** Current class (last loadout seen), null without an event. */
  class?: ClassId | null;
}

export interface SessionSummary {
  id: number;
  character_id: string;
  character_name: string;
  started_at: number;
  ended_at: number | null;
  kills: number;
  deaths: number;
  headshots: number;
  xp: number;
  data_version?: number;
  faction_id?: number | null;
}

export interface EventLine {
  t: number;
  kind: string;
  other_id: string;
  other_name: string;
  weapon: string;
  headshot: boolean;
  vehicle: string;
  team: number | null;
  enemy_class?: ClassId | null;
  my_class?: ClassId | null;
}

export interface Analysis {
  id: number | null;
  character_id: string;
  character_name: string;
  faction_id: number | null;
  started_at: number;
  ended_at: number | null;
  data_version: number;
  stats: SessionStats;
  rates: Rates;
  best_streak: number;
  weapons: { id: number; name: string; kills: number; headshots: number; hsr: number; deaths_by: number }[];
  vehicles: { id: number; name: string; kills: number }[];
  kills: EventLine[];
  deaths: EventLine[];
  support: { t: number; kind: string; other_id: string; other_name: string; detail: string }[];
  achievements: { id: string; name: string; description: string; timestamp: number }[];
  series: { minute: number; kills: number; deaths: number; headshots: number; xp: number; kd: number; hsr: number }[];
  teams: { team_id: number; kills: number; deaths: number }[];
  classes?: ClassStats;
}

export interface SquadRow {
  character_id: string;
  name: string;
  faction_id: number | null;
  own: boolean;
  online: boolean;
  kills: number;
  deaths: number;
  revives: number;
  kd: number;
  kpm: number;
  hsr: number;
  started_at: number | null;
  /** Last server/continent of the running session; names from Census (null while unknown). */
  world_id?: number | null;
  zone_id?: number | null;
  world_name?: string | null;
  zone_name?: string | null;
  class?: ClassId | null;
}

export interface OutfitMember {
  character_id: string;
  name: string;
  faction_id: number | null;
  online: boolean;
}

export interface UpdateStatus {
  available: boolean;
  /** "ok" | "no_key" | "flatpak" | "package" | "dev" */
  reason: string;
  current: string;
}

export interface UpdateInfo {
  version: string;
  date: string | null;
  notes: string;
}

export interface ImportOutcome {
  id: number;
  duplicate: boolean;
  character_name: string;
  started_at: number;
}

export interface EssStatus {
  state: "idle" | "connecting" | "connected" | "disconnected";
  detail: string;
  retry_in_secs: number | null;
  last_event_at: number | null;
}

export interface CharacterInfo {
  character_id: string;
  name: string;
  faction_id: number;
  world_id: number | null;
  battle_rank: number;
  prestige_level: number;
}

export interface MonitorInfo {
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  scale_factor: number;
  primary: boolean;
}

export interface PackMeta {
  source?: string;
  name: string;
  author: string;
  version: string;
  license: string;
  description: string;
}

export interface Conditions {
  min_count?: number | null;
  max_count?: number | null;
  headshot?: boolean | null;
  weapon_category?: string[];
  vehicle?: string[];
}

export interface PackEntry {
  event: string;
  files: string[];
  priority?: number | null;
  cooldown_ms: number;
  volume: number;
  when?: Conditions;
  silent?: boolean;
  overlay?: boolean;
  stream?: boolean;
}

export interface Manifest {
  pack: PackMeta;
  event: PackEntry[];
}

export interface VoicePack {
  id: string;
  dir: string;
  manifest: Manifest;
  warnings: string[];
}

export type PackFormat = "open" | "recursion";

export interface ImportResult {
  id: string;
  name: string;
  format: PackFormat;
  warnings: string[];
  source: string;
  events: number;
  stats: { embedded: number; loose: number; background: boolean; referenced: number; unreferenced: number };
}

export interface RemovedPack {
  id: string;
  trash: string;
  position: number | null;
  overrides: Record<string, string[]>;
}

export interface TriggerRule {
  event: string;
  on: string;
  description: string;
  count?: number;
  min_count?: number;
  headshot?: boolean;
  roadkill?: boolean;
  weapon_category?: string[];
  vehicle?: string[];
  session_count?: number;
  priority?: number;
  chain_style?: string;
}

export interface EventInfo {
  key: string;
  label: string;
  trigger: string | null;
  priority: number;
  recursion: boolean;
  rules: TriggerRule[];
  default_rules: TriggerRule[];
  last_played: number | null;
}

export interface PackInfo {
  id: string;
  meta: PackMeta;
  format: PackFormat;
  events: number;
  files: number;
  active: boolean;
  position: number | null;
  warnings: string[];
  background?: string | null;
  sounds?: string[];
  /** No PER_KILL sound: proposed key for "every kill". */
  suggested_kill_key?: string | null;
}

export interface VoiceOverview {
  packs: PackInfo[];
  invalid: [string, string][];
  events: EventInfo[];
  triggers_file: string;
}

export interface MappingProposal {
  file: string;
  event: string | null;
  count: number | null;
}

export interface CrosshairItem {
  file: string;
  width: number;
  height: number;
  data_url: string;
}

export interface FxEffect {
  enabled: boolean;
  mode: "tint" | "swap";
  color: string;
  file: string;
  duration_ms: number;
  fade: boolean;
  peak: number;
}

export type FxKind = "kill" | "headshot" | "assist" | "vehicle_kill" | "vehicle_damage";

export interface HitFeedback {
  enabled: boolean;
  kill: FxEffect;
  headshot: FxEffect;
  assist: FxEffect;
  vehicle_kill: FxEffect;
  vehicle_damage: FxEffect;
}

export interface CrosshairFx {
  kind: FxKind;
  effect: FxEffect;
  data_url: string | null;
}

export interface CrosshairRender {
  data_url: string;
  width: number;
  height: number;
  scale: number;
  opacity: number;
  tint: string;
}

export interface FocusInfo {
  available: boolean;
  class: string;
  title: string;
  matches: boolean;
  error: string;
  /** "game" | "other" | "unknown" – only "other" hides the crosshair */
  state: FocusState;
  watcher: FocusState;
  backend: string;
}

export type FocusState = "game" | "other" | "unknown";

export interface PathsInfo {
  config_file: string;
  achievements_file: string;
  database: string;
  voicepacks: string;
  crosshairs: string;
  log_file: string;
}

export interface Toast {
  title: string;
  text: string;
  kind: string;
}

/** Origin of the active Census service ID (never the value). */
export type ServiceIdSource = "environment" | "config" | "builtin" | "example";

/** Version of the running app (compile time). */
export interface AppInfo {
  version: string;
  commit: string;
  build_date: string;
}
