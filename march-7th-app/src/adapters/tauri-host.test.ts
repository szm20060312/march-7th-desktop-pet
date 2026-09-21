import { describe, expect, it } from "vitest";
import { parseCursorSample } from "./tauri-host";

describe("cursor IPC boundary", () => {
  it("accepts negative and fractional coordinates for multi-display/DPI use", () => {
    const sample = { x: -30.5, y: 8.25, windowX: -1920, windowY: -1080 };
    expect(parseCursorSample(sample)).toEqual(sample);
  });
  it.each([null, {}, { x: 0, y: 0, windowX: NaN, windowY: 0 },
    { x: Infinity, y: 0, windowX: 0, windowY: 0 }, { x: "1", y: 0, windowX: 0, windowY: 0 },
  ])("rejects malformed samples instead of poisoning animation state", value => {
    expect(() => parseCursorSample(value)).toThrow();
  });
});
