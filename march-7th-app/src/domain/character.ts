/** Internal, compiled-in character data; not an external character-pack format. */
export type AnimationClip = Readonly<{
  row: number;
  frameCount: number;
  frameIntervalMs: number;
}>;

export type OneShotAction = "wave" | "jump" | "offerWater";
export type ResponseContext = "click" | "doubleClick" | "reminderDue" | "reminderCompleted" | "reminderSnoozed" | "focusCompleted" | "taskCompleted" | "taskAbandoned";
export type ReminderTopic = "water" | "move" | "eyes" | "multiple";

export type WaterOffer = Readonly<{ kind?: "separate"; src: string; sourceWidth: number; sourceHeight: number; columns: 2; rows: 2; frameCount: 4; frameIntervalMs: number }>
  | Readonly<{ kind: "mainAtlas"; clip: AnimationClip }>;

export type CharacterPresentation = Readonly<{
  neutralFrame: Readonly<{ row: number; column: number }>;
  neutralHoldMs: number;
  movementTransitions: Readonly<{ left: AnimationClip; right: AnimationClip }>;
  /** Entry's last image must equal run frame zero before enabling this. */
  finishRunBeforeExit?: boolean;
}>;

export function waterOfferClip(offer: WaterOffer): AnimationClip {
  return offer.kind === "mainAtlas" ? offer.clip : { row: 0, frameCount: offer.frameCount, frameIntervalMs: offer.frameIntervalMs };
}

export function waterOfferFrame(offer: WaterOffer, index: number): Readonly<{ row: number; column: number; asset?: "waterOffer" }> {
  return offer.kind === "mainAtlas" ? { row: offer.clip.row, column: index }
    : { asset: "waterOffer", row: Math.floor(index / offer.columns), column: index % offer.columns };
}

export type CharacterDefinition = Readonly<{
  id: string;
  displayName: string;
  atlas: Readonly<{
    src: string;
    cellWidth: number;
    cellHeight: number;
    columns: number;
    rows: number;
    /** Bitmap pixels per logical pixel; omitted for existing 1x artwork. */
    pixelDensity?: 1 | 2;
  }>;
  clips: Readonly<{
    idle: AnimationClip;
    movingLeft: AnimationClip;
    movingRight: AnimationClip;
    wave?: AnimationClip;
    jump?: AnimationClip;
  }>;
  look: Readonly<{ firstRow: number; directionCount: number }>;
  waterOffer: WaterOffer;
  reminderPresentation?: Readonly<{ deferUntilReady: boolean; waterDuringFocus: boolean; waterHoldMs?: number }>;
  presentation?: CharacterPresentation;
  interaction?: Readonly<{ pointerRegion: Readonly<{ kind: "visibleBounds"; paddingPx: number }> }>;
  phrases: Readonly<Partial<Record<ResponseContext, readonly string[]>>>;
  reminderPrompts: Readonly<Record<ReminderTopic, readonly string[]>>;
}>;

export type CharacterCatalog = Readonly<{
  defaultId: string;
  characters: readonly CharacterDefinition[];
}>;

export function atlasBitmapDimensions(atlas: CharacterDefinition["atlas"]): Readonly<{ width: number; height: number }> {
  const density = atlas.pixelDensity ?? 1;
  return { width: atlas.cellWidth * atlas.columns * density, height: atlas.cellHeight * atlas.rows * density };
}
