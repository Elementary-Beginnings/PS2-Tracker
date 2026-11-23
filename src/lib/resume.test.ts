import { describe, expect, it } from "vitest";
import { canReopen, pauseLeft, playtime } from "./resume";

describe("resume", () => {
  it("counts playtime without pauses", () => {
    expect(playtime({ started_at: 100, ended_at: null, paused_at: null, paused_secs: 0 }, 400)).toBe(300);
    // relogin after 200 s logged out
    expect(playtime({ started_at: 100, ended_at: null, paused_at: null, paused_secs: 200 }, 400)).toBe(100);
    // paused: the clock stands still at the logout
    expect(playtime({ started_at: 100, ended_at: null, paused_at: 250, paused_secs: 0 }, 900)).toBe(150);
    // ended session, old data without the field
    expect(playtime({ started_at: 100, ended_at: 160 }, 900)).toBe(60);
  });
  it("shows the time left in the resume window", () => {
    expect(pauseLeft({ paused_at: null, resume_until: null }, 100)).toBeNull();
    expect(pauseLeft({ paused_at: 100, resume_until: 1000 }, 400)).toBe(600);
    expect(pauseLeft({ paused_at: 100, resume_until: 1000 }, 1200)).toBe(0);
  });
  it("offers the last session only within the limit", () => {
    const last = { id: 1, started_at: 0, ended_at: 100, resumable: true, resumable_until: 21700 };
    expect(canReopen(null, 200)).toBe(false);
    expect(canReopen(last, 200)).toBe(true);
    expect(canReopen(last, 21701)).toBe(false);
    expect(canReopen({ ...last, resumable: false }, 200)).toBe(false);
  });
});
