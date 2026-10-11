import { directionFrame, horizontalDirection, type CursorSample, type HorizontalDirection, type Point, type SpriteFrame } from "./animation";
import { waterOfferClip, waterOfferFrame, type AnimationClip, type CharacterDefinition, type CharacterPresentation, type OneShotAction } from "./character";

const FAST_RETURN_FRAME_LIMIT_MS = 20;
const ACTION_RETURN_BUDGET_MS = 160;
const TURN_RETURN_BUDGET_MS = 80;

function fastReturnInterval(clip: AnimationClip, budgetMs: number): number {
  return Math.min(FAST_RETURN_FRAME_LIMIT_MS, clip.frameIntervalMs, budgetMs / clip.frameCount);
}

type Request = Readonly<{ action: OneShotAction; onStarted?: () => void }>;
type Sequence = Readonly<{
  kind: "action" | "return" | "enter" | "exit" | "finishRun";
  clip: AnimationClip;
  action?: OneShotAction;
  from: number;
  step: 1 | -1;
  count: number;
  interval: number;
  startedAt: number;
  after: "neutral" | "enter" | "running" | "offer";
  exitClip?: AnimationClip;
  exitInterval?: number;
}>;

/** Opt-in presentation state. No native timing or character identity is used here. */
export class SequencedPlayback {
  private cursor: Point | null = null;
  private previousWindow: Point | null = null;
  private direction: HorizontalDirection = "right";
  private movingUntil = 0;
  private sequence: Sequence | null = null;
  private neutralUntil: number | null = null;
  private runningSince: number | null = null;
  private idleSince: number;
  private pending: Request | null = null;

  constructor(
    private readonly character: CharacterDefinition,
    private readonly presentation: CharacterPresentation,
    now: number,
    private readonly behavior: Readonly<{ lookDeadzonePx: number; movementThresholdPx: number; settleIntervalMs: number }>,
  ) { this.idleSince = now; }

  isMoving(now: number): boolean { return now < this.movingUntil; }

  acceptSample(sample: CursorSample, now: number): void {
    const position = { x: sample.windowX, y: sample.windowY };
    if (this.previousWindow) {
      const dx = position.x - this.previousWindow.x;
      const dy = position.y - this.previousWindow.y;
      const direction = horizontalDirection(dx, this.behavior.movementThresholdPx);
      if (direction || Math.abs(dy) > this.behavior.movementThresholdPx) this.move(now, direction ?? this.direction);
    }
    this.previousWindow = position;
    if (!this.isMoving(now)) this.cursor = { x: sample.x, y: sample.y };
  }

  sampleUnavailable(): void { this.cursor = null; }
  beginDrag(now: number, directionHint?: HorizontalDirection): void { this.move(now, directionHint ?? this.direction); }

  respond(action: OneShotAction, now: number, onStarted?: () => void): boolean {
    this.advance(now);
    if (this.isMoving(now) || this.runningSince !== null || this.sequence?.kind === "enter" || this.sequence?.kind === "exit" || this.sequence?.kind === "finishRun") return false;
    const request = { action, onStarted };
    if (this.sequence || this.neutralUntil !== null) {
      this.pending = request;
      if (this.sequence?.kind === "action" && this.sequence.action === "offerWater") this.returnOffer(now);
    } else this.startAction(request, now);
    return true;
  }

  /** Returns a duration only for the first end of an active cup presentation. */
  endOfferWater(now: number): number | null {
    this.advance(now);
    if (this.pending?.action === "offerWater") this.pending = null;
    return this.returnOffer(now);
  }

  private returnOffer(now: number): number | null {
    const active = this.sequence;
    if (active?.kind !== "action" || active.action !== "offerWater") return null;
    const from = this.indexAt(active, now);
    this.sequence = { ...active, kind: "return", from, step: -1, count: from + 1, startedAt: now, after: "neutral" };
    return (from + 1) * active.interval + this.presentation.neutralHoldMs;
  }

  frameAt(now: number, center: Point): SpriteFrame {
    this.advance(now);
    if (this.sequence) {
      const sequence = this.sequence;
      const column = this.indexAt(sequence, now);
      return sequence.action === "offerWater" ? waterOfferFrame(this.character.waterOffer, column) : { row: sequence.clip.row, column };
    }
    if (this.neutralUntil !== null) return this.presentation.neutralFrame;
    if (this.runningSince !== null) {
      const clip = this.movementClip();
      return { row: clip.row, column: Math.floor(Math.max(0, now - this.runningSince) / clip.frameIntervalMs) % clip.frameCount };
    }
    const gaze = this.cursor && directionFrame(this.cursor, center, this.behavior.lookDeadzonePx, this.character);
    if (gaze) return gaze;
    const idle = this.character.clips.idle;
    return { row: idle.row, column: Math.floor(Math.max(0, now - this.idleSince) / idle.frameIntervalMs) % idle.frameCount };
  }

  private indexAt(sequence: Sequence, now: number): number {
    const step = Math.min(sequence.count - 1, Math.floor(Math.max(0, now - sequence.startedAt) / sequence.interval));
    const column = sequence.from + sequence.step * step;
    return sequence.kind === "finishRun" ? column % sequence.clip.frameCount : column;
  }

  private startAction(request: Request, now: number): void {
    const clip = request.action === "offerWater" ? waterOfferClip(this.character.waterOffer)
      : this.character.clips[request.action] ?? this.character.clips.idle;
    this.sequence = { kind: "action", action: request.action, clip, from: 0, step: 1, count: clip.frameCount,
      interval: clip.frameIntervalMs, startedAt: now, after: request.action === "offerWater" ? "offer" : "neutral" };
    request.onStarted?.();
  }

  private enterMovement(now: number): void {
    this.neutralUntil = null;
    this.runningSince = null;
    if (!this.isMoving(now)) {
      this.neutralUntil = now + this.presentation.neutralHoldMs;
      return;
    }
    const clip = this.presentation.movementTransitions[this.direction];
    this.sequence = { kind: "enter", clip, from: 0, step: 1, count: clip.frameCount,
      interval: clip.frameIntervalMs, startedAt: now, after: "running" };
  }

  private exitMovement(now: number, from?: number, interval?: number, after: "neutral" | "enter" = "neutral", exitClip?: AnimationClip): void {
    const clip = exitClip ?? (this.sequence?.kind === "enter" ? this.sequence.clip : this.presentation.movementTransitions[this.direction]);
    const index = from ?? clip.frameCount - 1;
    this.runningSince = null;
    this.sequence = { kind: "exit", clip, from: index, step: -1, count: index + 1,
      interval: interval ?? clip.frameIntervalMs, startedAt: now, after };
  }

  private finishRun(now: number, after: "neutral" | "enter" = "neutral"): void {
    const clip = this.movementClip();
    const from = Math.floor(Math.max(0, now - this.runningSince!) / clip.frameIntervalMs) % clip.frameCount;
    const exitClip = this.presentation.movementTransitions[this.direction];
    const exitInterval = after === "enter" ? fastReturnInterval(exitClip, TURN_RETURN_BUDGET_MS) : exitClip.frameIntervalMs;
    if (from === 0) {
      this.exitMovement(now, undefined, exitInterval, after, exitClip);
      return;
    }
    this.runningSince = null;
    // Entry's last frame shares run frame zero. Reach that pose before changing
    // rows; ordinary stops keep the gait's timing, rapid turns have a bound.
    this.sequence = { kind: "finishRun", clip, from, step: 1, count: clip.frameCount - from + 1,
      interval: after === "enter" ? fastReturnInterval(clip, TURN_RETURN_BUDGET_MS) : clip.frameIntervalMs,
      startedAt: now, after, exitClip, exitInterval };
  }

  private move(now: number, direction: HorizontalDirection): void {
    // Drop queued gestures before advancing; a drag must never start queued feedback.
    this.pending = null;
    this.advance(now);
    const wasMoving = this.isMoving(now);
    const changedDirection = direction !== this.direction;
    this.movingUntil = now + this.behavior.settleIntervalMs;
    this.cursor = null;
    this.neutralUntil = null;
    const active = this.sequence;
    if (active?.kind === "finishRun") {
      const requestedClip = direction === "right" ? this.character.clips.movingRight : this.character.clips.movingLeft;
      const from = this.indexAt(active, now);
      if (requestedClip.row === active.clip.row) {
        this.sequence = null;
        this.runningSince = now - from * active.clip.frameIntervalMs;
      } else if (active.after !== "enter") {
        this.sequence = { ...active, from, count: active.clip.frameCount - from + 1,
          interval: fastReturnInterval(active.clip, TURN_RETURN_BUDGET_MS), startedAt: now, after: "enter",
          exitInterval: fastReturnInterval(active.exitClip!, TURN_RETURN_BUDGET_MS) };
      }
    } else if (active?.kind === "return" && active.after === "enter") {
      // IPC samples extend movement, not the already-running return's frame deadlines.
    } else if (active?.kind === "action" || active?.kind === "return") {
      const from = this.indexAt(active, now);
      this.sequence = { ...active, kind: "return", from, step: -1, count: from + 1,
        interval: Math.min(active.interval, fastReturnInterval(active.clip, ACTION_RETURN_BUDGET_MS)), startedAt: now, after: "enter" };
    } else if (changedDirection && this.runningSince !== null) {
      if (this.presentation.finishRunBeforeExit) this.finishRun(now, "enter");
      else this.exitMovement(now, undefined, fastReturnInterval(this.presentation.movementTransitions[this.direction], TURN_RETURN_BUDGET_MS), "enter");
    } else if (changedDirection && active?.kind === "enter") {
      this.exitMovement(now, this.indexAt(active, now),
        fastReturnInterval(active.clip, TURN_RETURN_BUDGET_MS), "enter");
    } else if (active?.kind === "exit" && active.after === "enter") {
      // Keep a direction change's return deadline even while further samples arrive.
    } else if (active?.kind === "exit" && changedDirection) {
      const from = this.indexAt(active, now);
      this.sequence = { ...active, from, count: from + 1, startedAt: now,
        interval: Math.min(active.interval, fastReturnInterval(active.clip, TURN_RETURN_BUDGET_MS)), after: "enter" };
    } else if (!wasMoving || active?.kind === "exit") {
      // Resume a partially completed exit from its current pose instead of snapping to frame zero.
      if (active?.kind === "exit" && !changedDirection) {
        const from = this.indexAt(active, now);
        this.sequence = { ...active, kind: "enter", from, step: 1, count: active.clip.frameCount - from,
          startedAt: now, interval: active.clip.frameIntervalMs, after: "running" };
      } else {
        this.direction = direction;
        this.enterMovement(now);
      }
    }
    this.direction = direction;
  }

  /** Resolve scheduled boundaries from their actual times, including delayed animation callbacks. */
  private advance(now: number): void {
    for (let boundary = 0; boundary < 12; boundary++) {
      const active = this.sequence;
      if (active) {
        const endedAt = active.startedAt + active.count * active.interval;
        if (active.kind === "enter" && now >= this.movingUntil && this.movingUntil >= active.startedAt
          && this.movingUntil < endedAt) {
          this.exitMovement(this.movingUntil, this.indexAt(active, this.movingUntil));
          continue;
        }
        if (active.after === "offer") return;
        if (now < endedAt) return;
        this.sequence = null;
        if (active.kind === "finishRun") this.exitMovement(endedAt, undefined, active.exitInterval,
          active.after === "enter" ? "enter" : "neutral", active.exitClip);
        else if (active.after === "enter") this.enterMovement(endedAt);
        else if (active.after === "running") this.runningSince = endedAt;
        else this.neutralUntil = endedAt + this.presentation.neutralHoldMs;
        continue;
      }
      if (this.runningSince !== null) {
        if (this.isMoving(now)) return;
        const endedAt = Math.max(this.runningSince, this.movingUntil);
        if (this.presentation.finishRunBeforeExit) this.finishRun(endedAt);
        else this.exitMovement(endedAt);
        continue;
      }
      if (this.neutralUntil !== null) {
        if (now < this.neutralUntil) return;
        const readyAt = this.neutralUntil;
        this.neutralUntil = null;
        this.idleSince = readyAt;
        const pending = this.pending;
        this.pending = null;
        if (pending) this.startAction(pending, readyAt);
        continue;
      }
      return;
    }
  }

  private movementClip(): AnimationClip { return this.direction === "right" ? this.character.clips.movingRight : this.character.clips.movingLeft; }
}
