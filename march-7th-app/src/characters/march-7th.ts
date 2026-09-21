import type { CharacterDefinition } from "../domain/character";

export const march7th: CharacterDefinition = {
  id: "march-7th",
  displayName: "三月七",
  atlas: {
    src: "/assets/march-7th/spritesheet.webp",
    cellWidth: 192,
    cellHeight: 208,
    columns: 8,
    rows: 11,
  },
  clips: {
    idle: { row: 0, frameCount: 6, frameIntervalMs: 280 },
    movingRight: { row: 1, frameCount: 8, frameIntervalMs: 90 },
    movingLeft: { row: 2, frameCount: 8, frameIntervalMs: 90 },
  },
  look: { firstRow: 9, directionCount: 16 },
};
