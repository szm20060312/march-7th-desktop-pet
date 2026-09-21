import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PetHost } from "../application/ports";
import type { CursorSample } from "../domain/animation";

// Validate the IPC boundary, not each consumer. A bad sample becomes unavailable.
export function parseCursorSample(value: unknown): CursorSample {
  if (typeof value !== "object" || value === null) throw new Error("Invalid cursor sample");
  const record = value as Record<string, unknown>;
  const coordinate = (key: string): number => {
    const number = record[key];
    if (typeof number !== "number" || !Number.isFinite(number)) throw new Error(`Invalid cursor coordinate: ${key}`);
    return number;
  };
  return { x: coordinate("x"), y: coordinate("y"), windowX: coordinate("windowX"), windowY: coordinate("windowY") };
}

export function createTauriHost(): PetHost {
  const appWindow = getCurrentWindow();
  return {
    sampleCursor: async () => parseCursorSample(await invoke<unknown>("cursor_relative_to_window")),
    startDragging: () => appWindow.startDragging(),
  };
}
