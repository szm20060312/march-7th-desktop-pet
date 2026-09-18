import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  IDLE_FRAME_COUNT,
  SPRITE_CELL_HEIGHT,
  SPRITE_CELL_WIDTH,
  directionFrame,
  horizontalDirection,
  type HorizontalDirection,
  type Point,
  type SpriteFrame,
} from "./animation";

type CursorSample = Point & {
  windowX: number;
  windowY: number;
};

const CURSOR_SAMPLE_INTERVAL_MS = 33;
const IDLE_FRAME_INTERVAL_MS = 280;
const LOOK_DEADZONE_PX = 20;
const MOVEMENT_FRAME_COUNT = 8;
const MOVEMENT_FRAME_INTERVAL_MS = 90;
const WINDOW_MOVEMENT_THRESHOLD_PX = 0.5;
const WINDOW_SETTLE_INTERVAL_MS = 160;

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Missing required element: ${selector}`);
  return element;
}

const sprite = requiredElement<HTMLElement>("#pet-sprite");
const stage = requiredElement<HTMLElement>("#pet-stage");
const appWindow = getCurrentWindow();

let latestCursor: Point | null = null;
let previousWindowPosition: Point | null = null;
let movementDirection: HorizontalDirection = "right";
let windowMovingUntil = 0;
let movementFrameIndex = 0;
let nextMovementFrameAt = 0;
let idleFrameIndex = 0;
let nextIdleFrameAt = performance.now() + IDLE_FRAME_INTERVAL_MS;
let lastRenderedFrame = "";

function renderFrame(frame: SpriteFrame): void {
  const frameKey = `${frame.row}:${frame.column}`;
  if (frameKey === lastRenderedFrame) return;

  sprite.style.backgroundPosition = [
    `${-frame.column * SPRITE_CELL_WIDTH}px`,
    `${-frame.row * SPRITE_CELL_HEIGHT}px`,
  ].join(" ");
  sprite.dataset.frame = frameKey;
  lastRenderedFrame = frameKey;
}

function render(now: number): void {
  if (now < windowMovingUntil) {
    if (now >= nextMovementFrameAt) {
      movementFrameIndex = (movementFrameIndex + 1) % MOVEMENT_FRAME_COUNT;
      nextMovementFrameAt = now + MOVEMENT_FRAME_INTERVAL_MS;
    }
    renderFrame({
      row: movementDirection === "right" ? 1 : 2,
      column: movementFrameIndex,
    });
    requestAnimationFrame(render);
    return;
  }

  const bounds = sprite.getBoundingClientRect();
  const lookFrame = latestCursor
    ? directionFrame(
        latestCursor,
        {
          x: bounds.left + bounds.width / 2,
          y: bounds.top + bounds.height / 2,
        },
        LOOK_DEADZONE_PX,
      )
    : null;

  if (lookFrame) {
    renderFrame(lookFrame);
  } else {
    if (now >= nextIdleFrameAt) {
      idleFrameIndex = (idleFrameIndex + 1) % IDLE_FRAME_COUNT;
      nextIdleFrameAt = now + IDLE_FRAME_INTERVAL_MS;
    }
    renderFrame({ row: 0, column: idleFrameIndex });
  }

  requestAnimationFrame(render);
}

function applyCursorSample(sample: CursorSample, now: number): void {
  const windowPosition = { x: sample.windowX, y: sample.windowY };

  if (previousWindowPosition) {
    const deltaX = windowPosition.x - previousWindowPosition.x;
    const deltaY = windowPosition.y - previousWindowPosition.y;
    const detectedDirection = horizontalDirection(
      deltaX,
      WINDOW_MOVEMENT_THRESHOLD_PX,
    );
    const windowMoved =
      detectedDirection !== null ||
      Math.abs(deltaY) > WINDOW_MOVEMENT_THRESHOLD_PX;

    if (windowMoved) {
      const movementWasInactive = now >= windowMovingUntil;
      if (detectedDirection && detectedDirection !== movementDirection) {
        movementDirection = detectedDirection;
        movementFrameIndex = 0;
        nextMovementFrameAt = now + MOVEMENT_FRAME_INTERVAL_MS;
      } else if (movementWasInactive) {
        movementFrameIndex = 0;
        nextMovementFrameAt = now + MOVEMENT_FRAME_INTERVAL_MS;
      }
      windowMovingUntil = now + WINDOW_SETTLE_INTERVAL_MS;
      latestCursor = null;
    }
  }

  previousWindowPosition = windowPosition;

  if (now >= windowMovingUntil) {
    latestCursor = { x: sample.x, y: sample.y };
  }
}

async function sampleCursor(): Promise<void> {
  try {
    const sample = await invoke<CursorSample>("cursor_relative_to_window");
    applyCursorSample(sample, performance.now());
    document.body.dataset.cursorTracking =
      performance.now() < windowMovingUntil ? "moving" : "active";
  } catch {
    latestCursor = null;
    document.body.dataset.cursorTracking = "unavailable";
  } finally {
    window.setTimeout(sampleCursor, CURSOR_SAMPLE_INTERVAL_MS);
  }
}

function startDragging(event: PointerEvent): void {
  if (event.button !== 0) return;

  latestCursor = null;
  windowMovingUntil = performance.now() + WINDOW_SETTLE_INTERVAL_MS;
  movementFrameIndex = 0;
  nextMovementFrameAt = performance.now() + MOVEMENT_FRAME_INTERVAL_MS;
  void appWindow.startDragging();
}

stage.addEventListener("pointerdown", startDragging);
renderFrame({ row: 0, column: 0 });
requestAnimationFrame(render);
void sampleCursor();
