import { describe, expect, it } from "vitest";
import { en } from "./i18n/en";
import type { ClassId } from "./types";

// the six classes the backend sends (crates/ps2-tracker-core/src/classes.rs)
const ALL: ClassId[] = ["infiltrator", "light_assault", "medic", "engineer", "heavy_assault", "max"];

describe("class labels", () => {
  it("has a name and a short form for every class", () => {
    for (const c of ALL) {
      expect(en.classes.names[c]).toBeTruthy();
      expect(en.classes.short[c]).toMatch(/^[A-Z]{2,3}$/);
    }
    expect(Object.keys(en.classes.names).sort()).toEqual([...ALL].sort());
    expect(en.classes.short.medic).toBe("MED");
  });
});
