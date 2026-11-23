import { describe, expect, it } from "vitest";
import { clampInto, sameArea, snap, toCss, toPhysical, type Rect } from "./movelayer";

/** A real layout: DP-1 5120×1440 at y=1200, below DP-2. */
const DP1: Rect = { x: 0, y: 1200, w: 5120, h: 1440 };
const on = { threshold: 12, grid: 0, enabled: true };

describe("move layer coordinates", () => {
  it("converts between absolute physical and layer CSS pixels with a monitor offset", () => {
    // scale 1: the layer starts at the monitor origin y=1200
    expect(toCss(1224, DP1.y, 1)).toBe(24);
    expect(toPhysical(24, DP1.y, 1)).toBe(1224);
    // scale 1.25 (logical 4096×1152): CSS 100 → 125 physical
    expect(toCss(1325, DP1.y, 1.25)).toBe(100);
    expect(toPhysical(100, DP1.y, 1.25)).toBe(1325);
    // round trip stays on whole physical pixels
    for (const dpr of [1, 1.25, 1.5, 2]) {
      for (const abs of [1200, 1201, 2000, 2639]) {
        expect(toPhysical(toCss(abs, DP1.y, dpr), DP1.y, dpr)).toBe(abs);
      }
    }
  });

  it("keeps objects on the monitor", () => {
    expect(clampInto({ x: -50, y: 0 }, { w: 400, h: 300 }, DP1)).toEqual({ x: 0, y: 1200 });
    expect(clampInto({ x: 9000, y: 9000 }, { w: 400, h: 300 }, DP1)).toEqual({ x: 4720, y: 2340 });
  });

  it("matches the layer monitor", () => {
    expect(sameArea([0, 1200, 5120, 1440], { x: 0, y: 1200, width: 5120, height: 1440 })).toBe(true);
    expect(sameArea([1280, 0, 2560, 1200], { x: 0, y: 1200, width: 5120, height: 1440 })).toBe(false);
  });
});

describe("snapping", () => {
  const size = { w: 400, h: 300 };

  it("snaps to monitor edges and centre", () => {
    const r = snap({ x: 4712, y: 1207 }, size, DP1, [], on);
    expect([r.x, r.y]).toEqual([4720, 1200]);
    expect(r.guidesX).toEqual([5120]);
    expect(r.guidesY).toEqual([1200]);
    const c = snap({ x: 2365, y: 1775 }, size, DP1, [], on);
    expect([c.x, c.y]).toEqual([2360, 1770]);
    expect(c.guidesX).toEqual([2560]);
  });

  it("snaps to the other object", () => {
    const popup: Rect = { x: 2000, y: 1500, w: 420, h: 100 };
    // just below the pop-up, left edges aligned
    const r = snap({ x: 2006, y: 1608 }, size, DP1, [popup], on);
    expect([r.x, r.y]).toEqual([2000, 1600]);
    // right next to it
    const s = snap({ x: 2425, y: 1503 }, size, DP1, [popup], on);
    expect([s.x, s.y]).toEqual([2420, 1500]);
  });

  it("Alt switches snapping and grid off; the grid applies where nothing snaps", () => {
    expect(snap({ x: 4712, y: 1207 }, size, DP1, [], { ...on, enabled: false })).toMatchObject({ x: 4712, y: 1207 });
    const g = snap({ x: 1234, y: 1555 }, size, DP1, [], { ...on, grid: 10 });
    expect([g.x, g.y]).toEqual([1230, 1560]);
    expect(snap({ x: 1234, y: 1555 }, size, DP1, [], { ...on, grid: 10, enabled: false })).toMatchObject({ x: 1234, y: 1555 });
  });

  it("never leaves the monitor", () => {
    expect(snap({ x: -500, y: 5000 }, size, DP1, [], on)).toMatchObject({ x: 0, y: 2340 });
  });
});
