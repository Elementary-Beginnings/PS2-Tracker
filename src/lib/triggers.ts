// Trigger templates for the Events page (same facts as triggers::FACTS in Rust).
import { t } from "./i18n";
import type { TriggerRule } from "./types";

export const FACTS = [
  "kill",
  "first_kill",
  "multikill",
  "headshot_streak",
  "repeat_victim",
  "revenge",
  "mutual",
  "streak",
  "streak_ended",
  "death",
  "nemesis",
  "death_streak",
  "suicide",
  "teamkill",
  "teamkilled",
  "revive",
  "revived",
  "vehicle_kill",
  "achievement",
  "login",
  "session_start",
];

/** Facts that carry a count (chain length, streak, number in the session). */
export const COUNTED_FACTS = ["multikill", "headshot_streak", "repeat_victim", "streak", "streak_ended", "nemesis", "death_streak"];
/** Facts that are a kill (headshot, roadkill, weapon category, own vehicle apply). */
export const KILL_FACTS = ["kill", "first_kill", "multikill", "revenge", "mutual"];

/** Plain-English summary of a rule, used when the user writes no description. */
export function describeRule(r: TriggerRule): string {
  const T = t.voice.trig;
  const parts = [t.voice.facts[r.on] ?? r.on];
  if (r.count != null) parts.push(T.dCount(r.count));
  if (r.min_count != null) parts.push(T.dMin(r.min_count));
  if (r.headshot != null) parts.push(r.headshot ? T.dHs : T.dNoHs);
  if (r.roadkill != null) parts.push(r.roadkill ? T.dRoad : T.dNoRoad);
  if (r.weapon_category?.length) parts.push(T.dWeapon(r.weapon_category.join(", ")));
  if (r.vehicle?.length) parts.push(T.dVehicle(r.vehicle.join(", ")));
  if (r.session_count != null) parts.push(T.dNth(r.session_count));
  return parts.join(", ");
}
