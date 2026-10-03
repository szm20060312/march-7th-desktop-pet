import type { CharacterDefinition } from "./character";

export type Point = Readonly<{ x: number; y: number }>;
export type SpriteFrame = Readonly<{ row: number; column: number; asset?: "waterOffer" }>;
export type HorizontalDirection = "left" | "right";
export type CursorSample = Point & Readonly<{ windowX: number; windowY: number }>;

export function horizontalDirection(deltaX: number, thresholdPx: number): HorizontalDirection | null {
  if (deltaX > thresholdPx) return "right";
  if (deltaX < -thresholdPx) return "left";
  return null;
}

export function directionFrame(
  target: Point,
  center: Point,
  deadzonePx: number,
  character: CharacterDefinition,
): SpriteFrame | null {
  const dx = target.x - center.x;
  const dy = target.y - center.y;
  if (Math.hypot(dx, dy) <= deadzonePx) return null;

  const degrees = (Math.atan2(dx, -dy) * (180 / Math.PI) + 360) % 360;
  const { directionCount, firstRow } = character.look;
  const index = Math.round(degrees / (360 / directionCount)) % directionCount;
  return {
    row: firstRow + Math.floor(index / character.atlas.columns),
    column: index % character.atlas.columns,
  };
}
