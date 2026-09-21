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
  onDragStart(handler: () => void): () => void;
}
export interface Scheduler {
  now(): number;
  requestFrame(callback: (now: number) => void): number;
  cancelFrame(id: number): void;
  setDelay(callback: () => void, ms: number): number;
  cancelDelay(id: number): void;
}
