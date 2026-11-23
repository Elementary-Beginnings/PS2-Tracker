import { describe, expect, it } from "vitest";
import { placeMenu } from "./menu";

const vp = { width: 1100, height: 700 };
const menu = { width: 260, height: 120 };

describe("placeMenu", () => {
  it("opens below, left-aligned to the trigger", () => {
    expect(placeMenu({ left: 240, right: 330, top: 100, bottom: 130 }, menu, vp)).toEqual({ left: 240, top: 134 });
  });
  it("flips to the right edge of the trigger near the right border", () => {
    // "…" button of the right-most pack card
    expect(placeMenu({ left: 1050, right: 1080, top: 200, bottom: 228 }, menu, vp)).toEqual({ left: 820, top: 232 });
  });
  it("never leaves the viewport on the left", () => {
    expect(placeMenu({ left: 2, right: 30, top: 0, bottom: 20 }, { width: 1200, height: 50 }, vp).left).toBe(8);
  });
  it("opens upwards when there is no room below", () => {
    expect(placeMenu({ left: 100, right: 160, top: 640, bottom: 670 }, menu, vp).top).toBe(516);
  });
});
