import { describe, expect, it } from "vitest";
import { directionFrame, horizontalDirection } from "./domain/animation";

import { march7th } from "./characters/march-7th";

const center = { x: 100, y: 100 };

describe("directionFrame", () => {
  it.each([
    ["up", { x: 100, y: 0 }, { row: 9, column: 0 }],
    ["right", { x: 200, y: 100 }, { row: 9, column: 4 }],
    ["down", { x: 100, y: 200 }, { row: 10, column: 0 }],
    ["left", { x: 0, y: 100 }, { row: 10, column: 4 }],
    ["up-right", { x: 200, y: 0 }, { row: 9, column: 2 }],
    ["down-left", { x: 0, y: 200 }, { row: 10, column: 2 }],
  ])("maps %s to the expected v2 frame", (_label, target, expected) => {
    expect(directionFrame(target, center, 10, march7th)).toEqual(expected);
  });

  it("returns idle inside the deadzone", () => {
    expect(directionFrame({ x: 108, y: 106 }, center, 10, march7th)).toBeNull();
  });
});

describe("horizontalDirection", () => {
  it("detects movement to the right", () => {
    expect(horizontalDirection(4, 0.5)).toBe("right");
  });

  it("detects movement to the left", () => {
    expect(horizontalDirection(-4, 0.5)).toBe("left");
  });

  it("ignores horizontal noise below the threshold", () => {
    expect(horizontalDirection(0.25, 0.5)).toBeNull();
  });
});
