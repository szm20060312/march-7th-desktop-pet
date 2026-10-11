import type { Point } from "./animation";

/** Cell coordinates, right and bottom exclusive. Only compact rectangles are retained. */
export type VisibleBounds = Readonly<{ left: number; top: number; right: number; bottom: number }>;
export type CharacterFrameBounds = Readonly<{
  main: readonly (VisibleBounds | null)[];
  waterOffer?: readonly (VisibleBounds | null)[];
}>;

export function visiblePixelBounds(rgba: Uint8ClampedArray, width: number, height: number): VisibleBounds | null {
  if (rgba.length !== width * height * 4) throw new Error("Invalid pointer-region pixels");
  let left = width; let top = height; let right = 0; let bottom = 0;
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    // Exclude almost invisible interpolation fringes. The logical margin covers antialiasing.
    if (rgba[(y * width + x) * 4 + 3] < 16) continue;
    left = Math.min(left, x); top = Math.min(top, y);
    right = Math.max(right, x + 1); bottom = Math.max(bottom, y + 1);
  }
  return right > left && bottom > top ? { left, top, right, bottom } : null;
}

export function containsVisiblePoint(bounds: VisibleBounds | null, point: Point, padding: number): boolean {
  return bounds !== null && Number.isFinite(point.x) && Number.isFinite(point.y) &&
    point.x >= bounds.left - padding && point.x < bounds.right + padding &&
    point.y >= bounds.top - padding && point.y < bounds.bottom + padding;
}
