import { invoke } from "@tauri-apps/api/core";
import {
  IDLE_FRAME_COUNT,
  SPRITE_CELL_HEIGHT,
  SPRITE_CELL_WIDTH,
  directionFrame,
  type SpriteFrame,
} from "./animation";

type CursorPosition = {
  x: number;
  y: number;
};

const CURSOR_SAMPLE_INTERVAL_MS = 33;
const IDLE_FRAME_INTERVAL_MS = 280;
const LOOK_DEADZONE_PX = 20;

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Missing required element: ${selector}`);
  return element;
}

const sprite = requiredElement<HTMLElement>("#pet-sprite");

let latestCursor: CursorPosition | null = null;
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

async function sampleCursor(): Promise<void> {
  try {
    latestCursor = await invoke<CursorPosition>("cursor_relative_to_window");
    document.body.dataset.cursorTracking = "active";
  } catch {
    latestCursor = null;
    document.body.dataset.cursorTracking = "unavailable";
  } finally {
    window.setTimeout(sampleCursor, CURSOR_SAMPLE_INTERVAL_MS);
  }
}

renderFrame({ row: 0, column: 0 });
requestAnimationFrame(render);
void sampleCursor();
