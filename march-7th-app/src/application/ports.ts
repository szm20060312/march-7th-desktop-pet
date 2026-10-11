import type { CursorSample, HorizontalDirection, Point, SpriteFrame } from "../domain/animation";
import type { CharacterDefinition } from "../domain/character";

export interface PetHost {
  sampleCursor(): Promise<CursorSample>;
  startDragging(): Promise<void>;
  /** Only offered by hosts supporting native, selective pointer passthrough. */
  setPointerPassthrough?(ignore: boolean, characterId: string, enabled?: boolean): Promise<boolean>;
}
export interface PetView {
  configure(character: CharacterDefinition): void;
  center(): Point;
  render(frame: SpriteFrame): void;
  setTracking(status: "active" | "moving" | "unavailable"): void;
  showPhrase(text: string): void;
  clearPhrase(): void;
  setPresentationError(message: string | null): void;
  setPointerRegionEnabled?(enabled: boolean): void;
  containsPointer?(point: Point): boolean;
}
export interface PetGestureHandlers {
  click(): void;
  doubleClick(): void;
  drag(directionHint?: HorizontalDirection): void;
}
export interface PetGestureInput {
  subscribe(handlers: PetGestureHandlers, policy?: Readonly<{ accepts(point: Point): boolean }>): () => void;
}
export interface Scheduler {
  now(): number;
  requestFrame(callback: (now: number) => void): number;
  cancelFrame(id: number): void;
  setDelay(callback: () => void, ms: number): number;
  cancelDelay(id: number): void;
}

export interface AtlasSize { width: number; height: number }
export interface AtlasPreloader { (src: string): Promise<AtlasSize> }
export interface PresentationStatus { setPresentationError(message: string | null): void }
