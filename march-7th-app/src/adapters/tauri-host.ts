import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PetHost } from "../application/ports";
import type { CursorSample } from "../domain/animation";

let pointerSequence = 0;

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
  let pointerQueue: Promise<unknown> = Promise.resolve();
  const windowsPointerHost = typeof navigator !== "undefined" && /^Win/.test(navigator.platform);
  return {
    sampleCursor: async () => parseCursorSample(await invoke<unknown>("cursor_relative_to_window")),
    startDragging: () => appWindow.startDragging(),
    ...(windowsPointerHost ? {
      setPointerPassthrough(ignore: boolean, characterId: string, enabled = true): Promise<boolean> {
        pointerSequence = Math.max(pointerSequence + 1, Date.now() * 1000);
        const sequence = pointerSequence;
        // One queue belongs to the host, not an animation lifecycle. A stopped
        // character's restore cannot overtake its pending native request.
        const result = pointerQueue.catch(() => undefined).then(async () => {
          const applied = await invoke<unknown>("set_pet_pointer_passthrough", { ignore, characterId, enabled, sequence });
          if (typeof applied !== "boolean") throw new Error("Invalid native pointer policy result");
          return applied;
        });
        pointerQueue = result;
        return result;
      },
    } : {}),
  };
}
