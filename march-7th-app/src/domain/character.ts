/** Internal, compiled-in character data; not an external character-pack format. */
export type AnimationClip = Readonly<{
  row: number;
  frameCount: number;
  frameIntervalMs: number;
}>;

export type OneShotAction = "wave" | "jump";
export type ResponseContext = "click" | "doubleClick" | "reminderCompleted" | "reminderSnoozed" | "focusCompleted" | "taskCompleted" | "taskAbandoned";

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
    wave?: AnimationClip;
    jump?: AnimationClip;
  }>;
  look: Readonly<{ firstRow: number; directionCount: number }>;
  phrases: Readonly<Partial<Record<ResponseContext, readonly string[]>>>;
}>;

export type CharacterCatalog = Readonly<{
  defaultId: string;
  characters: readonly CharacterDefinition[];
}>;
