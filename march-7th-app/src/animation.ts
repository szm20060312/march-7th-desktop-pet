export const SPRITE_CELL_WIDTH = 192;
export const SPRITE_CELL_HEIGHT = 208;
export const SPRITE_COLUMNS = 8;
export const SPRITE_ROWS = 11;
export const IDLE_FRAME_COUNT = 6;

const LOOK_STEP_DEGREES = 22.5;
const LOOK_DIRECTION_COUNT = 16;
const LOOK_FIRST_ROW = 9;

export type Point = {
  x: number;
  y: number;
};

export type SpriteFrame = {
  row: number;
  column: number;
};

export function directionFrame(
  target: Point,
  center: Point,
  deadzonePx: number,
): SpriteFrame | null {
  const dx = target.x - center.x;
  const dy = target.y - center.y;

  if (Math.hypot(dx, dy) <= deadzonePx) return null;

  const clockwiseDegrees =
    (Math.atan2(dx, -dy) * (180 / Math.PI) + 360) % 360;
  const directionIndex =
    Math.round(clockwiseDegrees / LOOK_STEP_DEGREES) % LOOK_DIRECTION_COUNT;

  return {
    row: LOOK_FIRST_ROW + Math.floor(directionIndex / SPRITE_COLUMNS),
    column: directionIndex % SPRITE_COLUMNS,
  };
}

