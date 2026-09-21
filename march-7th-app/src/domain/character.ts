/** Internal, compiled-in character data; not an external character-pack format. */
export type AnimationClip = Readonly<{
  row: number;
  frameCount: number;
  frameIntervalMs: number;
}>;

export type CharacterDefinition = Readonly<{
  id: string;
  displayName: string;
  atlas: Readonly<{
    src: string;
    cellWidth: number;
    cellHeight: number;
    columns: number;
    rows: number;
  }>;
  clips: Readonly<{
    idle: AnimationClip;
    movingLeft: AnimationClip;
    movingRight: AnimationClip;
  }>;
  look: Readonly<{ firstRow: number; directionCount: number }>;
}>;
