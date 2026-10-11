import { describe, expect, it } from "vitest";
import { containsVisiblePoint, visiblePixelBounds } from "./pointer-region";

describe("visible pointer region", () => {
  it("measures alpha without treating invisible RGB or faint fringes as body pixels", () => {
    const pixels = new Uint8ClampedArray(4 * 3 * 4);
    pixels[3] = 15;
    pixels[(1 * 4 + 1) * 4 + 3] = 16;
    pixels[(2 * 4 + 2) * 4 + 3] = 255;
    pixels[(0 * 4 + 3) * 4] = 255;
    expect(visiblePixelBounds(pixels, 4, 3)).toEqual({ left: 1, top: 1, right: 3, bottom: 3 });
    expect(visiblePixelBounds(new Uint8ClampedArray(48), 4, 3)).toBeNull();
  });
  it("keeps four logical pixels around the measured bounds and rejects invalid cursor values", () => {
    const bounds = { left: 50, top: 10, right: 145, bottom: 202 };
    expect(containsVisiblePoint(bounds, { x: 46, y: 6 }, 4)).toBe(true);
    expect(containsVisiblePoint(bounds, { x: 45, y: 10 }, 4)).toBe(false);
    expect(containsVisiblePoint(bounds, { x: 149, y: 10 }, 4)).toBe(false);
    expect(containsVisiblePoint(bounds, { x: 50, y: Infinity }, 4)).toBe(false);
    expect(containsVisiblePoint(null, { x: 0, y: 0 }, 4)).toBe(false);
  });
});
