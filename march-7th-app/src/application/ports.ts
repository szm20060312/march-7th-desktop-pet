import type { CursorSample, Point, SpriteFrame } from "../domain/animation";
import type { CharacterDefinition } from "../domain/character";

export interface PetHost {
  sampleCursor(): Promise<CursorSample>;
  startDragging(): Promise<void>;
}
export interface PetView {
  configure(character: CharacterDefinition): void;
  center(): Point;
  render(frame: SpriteFrame): void;
  setTracking(status: "active" | "moving" | "unavailable"): void;
  showPhrase(text: string): void;
  clearPhrase(): void;
  setPresentationError(message: string | null): void;
}
export interface PetGestureHandlers {
  click(): void;
  doubleClick(): void;
  drag(): void;
}
export interface PetGestureInput {
  subscribe(handlers: PetGestureHandlers): () => void;
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
