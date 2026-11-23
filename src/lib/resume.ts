// Pause and resume of sessions: pure helpers for the Live page and the session view.
import type { LastSession, Snapshot } from "./types";

/** Playtime in seconds: wall time since the start without the paused time; the
 *  clock stands still while logged out. */
export function playtime(s: Pick<Snapshot, "started_at" | "ended_at" | "paused_at" | "paused_secs">, now: number): number {
  const end = s.ended_at ?? s.paused_at ?? now;
  return Math.max(0, end - s.started_at - (s.paused_secs ?? 0));
}

/** Seconds left in the resume window of a paused session, or null if it is not paused. */
export function pauseLeft(s: Pick<Snapshot, "paused_at" | "resume_until">, now: number): number | null {
  if (s.paused_at == null) return null;
  if (s.resume_until == null) return 0;
  return Math.max(0, s.resume_until - now);
}

/** Can "Resume last session" open this session now? The server decides again
 *  on the click; this only drives the button (the clock keeps running while the
 *  page is open). */
export function canReopen(last: LastSession | null, now: number): boolean {
  return !!last && last.resumable && now <= last.resumable_until;
}
