import { describe, expect, it } from "vitest";
import { fxAlpha, recolor } from "./crosshair";

describe("recolor", () => {
  it("keeps brightness and alpha, replaces the hue", () => {
    const px = new Uint8ClampedArray([0, 255, 0, 255, 20, 20, 20, 128]);
    recolor(px, 255, 0, 0);
    expect([...px]).toEqual([255, 0, 0, 255, 20, 0, 0, 128]);
  });
});

describe("hit feedback timing", () => {
  it("holds, then fades, then ends", () => {
    expect(fxAlpha(0, 150, true)).toBe(1);
    expect(fxAlpha(40, 150, true)).toBe(1); // hold = 45 ms
    expect(fxAlpha(97.5, 150, true)).toBeCloseTo(0.5, 5);
    expect(fxAlpha(150, 150, true)).toBe(0);
    expect(fxAlpha(-1, 150, true)).toBe(0);
  });
  it("switches back at once without fade", () => {
    expect(fxAlpha(149, 150, false)).toBe(1);
    expect(fxAlpha(150, 150, false)).toBe(0);
  });
  it("keeps at least 30 ms at full strength", () => {
    expect(fxAlpha(29, 40, true)).toBe(1);
  });
});
