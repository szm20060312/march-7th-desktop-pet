import { afterEach, describe, expect, it, vi } from "vitest";
import { march7th } from "../characters/march-7th";
import { loadBrowserFrameBounds } from "./browser-frame-bounds";

afterEach(() => vi.unstubAllGlobals());

describe("compact browser frame bounds", () => {
  it("samples one cell at a time, caches rectangles, and releases the temporary canvas pixels", async () => {
    const images: { src: string }[] = [];
    vi.stubGlobal("Image", class {
      src = ""; naturalWidth = 8; naturalHeight = 6;
      constructor() { images.push(this); }
      async decode() { return; }
    });
    let sourceX = 0; let sourceY = 0;
    const draw = vi.fn((_image, x, y) => { sourceX = x; sourceY = y; });
    const context = {
      clearRect: vi.fn(), drawImage: draw,
      getImageData: vi.fn(() => {
        const data = new Uint8ClampedArray(4 * 3 * 4);
        if (sourceY === 0) data[(sourceX === 0 ? 1 * 4 + 1 : 2 * 4 + 2) * 4 + 3] = 255;
        return { data };
      }),
    };
    const canvas = { width: 0, height: 0, getContext: vi.fn(() => context) };
    vi.stubGlobal("document", { createElement: vi.fn(() => canvas) });
    const character = { ...march7th, atlas: { src: "/bounds-small.webp", columns: 2, rows: 2, cellWidth: 4, cellHeight: 3 }, waterOffer: { kind: "mainAtlas" as const, clip: { row: 0, frameCount: 2, frameIntervalMs: 10 } } };
    const bounds = await loadBrowserFrameBounds(character);
    expect(bounds.main).toEqual([
      { left: 1, top: 1, right: 2, bottom: 2 },
      { left: 2, top: 2, right: 3, bottom: 3 }, null, null,
    ]);
    expect(draw).toHaveBeenCalledTimes(4);
    expect(draw).toHaveBeenLastCalledWith(images[0], 4, 3, 4, 3, 0, 0, 4, 3);
    expect(context.getImageData).toHaveBeenCalledWith(0, 0, 4, 3);
    expect(canvas.width).toBe(0); expect(canvas.height).toBe(0);
    expect(images[0].src).toBe("");
    expect(await loadBrowserFrameBounds(character)).toBe(bounds);
    expect(draw).toHaveBeenCalledTimes(4);
  });
});
