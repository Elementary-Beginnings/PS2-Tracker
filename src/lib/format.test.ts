import { describe, expect, it } from "vitest";
import { ago, clock, dateTime, delta, duration, faction, feedLabel, locationLabel, num, pct } from "./format";

describe("format", () => {
  it("formats relative times", () => {
    expect(ago(100, 112)).toBe("12 s ago");
    expect(ago(0, 300)).toBe("5 min ago");
    expect(ago(0, 7300)).toBe("2 h ago");
  });
  it("formats durations", () => {
    expect(duration(59)).toBe("0:59");
    expect(duration(3725)).toBe("1:02:05");
    expect(duration(-3)).toBe("0:00");
  });
  it("formats numbers in English", () => {
    expect(num(1.2234, 2)).toBe("1.22");
    expect(num(1234.5, 1)).toBe("1,234.5");
    expect(pct(0.254)).toBe("25.4 %");
    expect(num(Number.NaN)).toBe("0");
  });
  it("formats differences", () => {
    expect(delta(12.4)).toBe("+12 %");
    expect(delta(-5)).toBe("−5 %");
    expect(delta(0)).toBe("±0 %");
    expect(delta(null)).toBe("–");
  });
  it("formats date and time with a 24-hour clock", () => {
    const ts = new Date(2026, 10, 20, 21, 7, 5).getTime() / 1000;
    expect(dateTime(ts)).toBe("2026-11-20 21:07");
    expect(clock(ts)).toBe("21:07:05");
  });
  it("knows factions and feed labels", () => {
    expect(faction(3).short).toBe("TR");
    expect(faction(3).name).toBe("Terran Republic");
    expect(faction(null).short).toBe("?");
    expect(feedLabel("vehicle_lost")).toBe("Vehicle lost");
  });
});

describe("locationLabel", () => {
  it("prefers Census names, falls back to the server list, leaves out unknown parts", () => {
    expect(locationLabel({ world_id: 10, zone_id: 2, world_name: "Miller", zone_name: "Indar" })).toBe("Miller / Indar");
    expect(locationLabel({ world_id: 10, zone_id: 2 })).toBe("Wainwright (Miller) / Indar");
    expect(locationLabel({ world_id: 10, zone_id: 99999 })).toBe("Wainwright (Miller)");
    expect(locationLabel({ world_id: null, zone_id: null })).toBe("");
    expect(locationLabel({ zone_name: "Esamir" })).toBe("Esamir");
  });
});
