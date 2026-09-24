/** Internal, compiled-in character data; not an external character-pack format. */
export type AnimationClip = Readonly<{
  row: number;
  frameCount: number;
  frameIntervalMs: number;
}>;

export type OneShotAction = "wave" | "jump" | "offerWater";
export type ResponseContext = "click" | "doubleClick" | "reminderDue" | "reminderCompleted" | "reminderSnoozed" | "focusCompleted" | "taskCompleted" | "taskAbandoned";
export type ReminderTopic = "water" | "move" | "eyes" | "multiple";

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
  waterOffer: Readonly<{ src: string; sourceWidth: number; sourceHeight: number; columns: 2; rows: 2; frameCount: 4; frameIntervalMs: number }>;
  phrases: Readonly<Partial<Record<ResponseContext, readonly string[]>>>;
  reminderPrompts: Readonly<Record<ReminderTopic, readonly string[]>>;
}>;

export type CharacterCatalog = Readonly<{
  defaultId: string;
  characters: readonly CharacterDefinition[];
}>;
