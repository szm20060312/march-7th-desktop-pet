import { directionFrame, horizontalDirection, type CursorSample, type HorizontalDirection, type Point, type SpriteFrame } from "./animation";
import type { AnimationClip, CharacterDefinition, OneShotAction } from "./character";

export const DEFAULT_PET_BEHAVIOR = Object.freeze({
  lookDeadzonePx: 20,
  movementThresholdPx: 0.5,
  settleIntervalMs: 160,
});
type PetBehavior = typeof DEFAULT_PET_BEHAVIOR;

/** Owns one pet's animation state. All time and coordinates are supplied by callers. */
export class PetModel {
  private cursor: Point | null = null;
  private previousWindow: Point | null = null;
  private direction: HorizontalDirection = "right";
  private movingUntil = 0;
  private movementFrame = 0;
  private nextMovementFrameAt = 0;
  private idleFrame = 0;
  private nextIdleFrameAt: number;
  private oneShot: Readonly<{ clip: AnimationClip; startedAt: number }> | null = null;

  constructor(
    private readonly character: CharacterDefinition,
    now: number,
    private readonly behavior: PetBehavior = DEFAULT_PET_BEHAVIOR,
  ) {
    this.nextIdleFrameAt = now + character.clips.idle.frameIntervalMs;
  }

  isMoving(now: number): boolean { return now < this.movingUntil; }

  acceptSample(sample: CursorSample, now: number): void {
    const position = { x: sample.windowX, y: sample.windowY };
    if (this.previousWindow) {
      const dx = position.x - this.previousWindow.x;
      const dy = position.y - this.previousWindow.y;
      const direction = horizontalDirection(dx, this.behavior.movementThresholdPx);
      if (direction !== null || Math.abs(dy) > this.behavior.movementThresholdPx) {
        this.oneShot = null;
        if (direction && direction !== this.direction) {
          this.direction = direction;
          this.resetMovementClip(now);
        } else if (!this.isMoving(now)) {
          this.resetMovementClip(now);
        }
        this.movingUntil = now + this.behavior.settleIntervalMs;
        this.cursor = null;
      }
    }
    this.previousWindow = position;
    if (!this.isMoving(now)) this.cursor = { x: sample.x, y: sample.y };
  }

  sampleUnavailable(): void { this.cursor = null; }

  beginDrag(now: number): void {
    this.oneShot = null;
    this.cursor = null;
    this.movingUntil = now + this.behavior.settleIntervalMs;
    this.resetMovementClip(now);
  }

  frameAt(now: number, center: Point): SpriteFrame {
    if (this.isMoving(now)) {
      const clip = this.movementClip();
      if (now >= this.nextMovementFrameAt) {
        this.movementFrame = (this.movementFrame + 1) % clip.frameCount;
        this.nextMovementFrameAt = now + clip.frameIntervalMs;
      }
      return { row: clip.row, column: this.movementFrame };
    }
    if (this.oneShot) {
      const { clip } = this.oneShot;
      const column = Math.floor(Math.max(0, now - this.oneShot.startedAt) / clip.frameIntervalMs);
      if (column < clip.frameCount) return { row: clip.row, column };
      this.oneShot = null;
    }
    const look = this.cursor && directionFrame(this.cursor, center, this.behavior.lookDeadzonePx, this.character);
    if (look) return look;

    const clip = this.character.clips.idle;
    if (now >= this.nextIdleFrameAt) {
      this.idleFrame = (this.idleFrame + 1) % clip.frameCount;
      this.nextIdleFrameAt = now + clip.frameIntervalMs;
    }
    return { row: clip.row, column: this.idleFrame };
  }

  respond(action: OneShotAction, now: number): boolean {
    if (this.isMoving(now)) return false;
    this.oneShot = { clip: this.character.clips[action] ?? this.character.clips.idle, startedAt: now };
    return true;
  }

  private movementClip() {
    return this.direction === "right" ? this.character.clips.movingRight : this.character.clips.movingLeft;
  }

  private resetMovementClip(now: number): void {
    this.movementFrame = 0;
    this.nextMovementFrameAt = now + this.movementClip().frameIntervalMs;
  }
}
