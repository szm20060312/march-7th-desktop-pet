import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface DebugLogEntry {
  timestamp: string;
  category: string;
  message: string;
}

export type DebugPetAction =
  | "wave"
  | "jump"
  | "offerWater"
  | "endWater"
  | "phrase"
  | "reminderPrompt:water"
  | "reminderPrompt:move"
  | "reminderPrompt:eyes";

export async function getDebugLogs(): Promise<DebugLogEntry[]> {
  return invoke<DebugLogEntry[]>("get_debug_logs");
}

export async function clearDebugLogs(): Promise<void> {
  return invoke<void>("clear_debug_logs");
}

export async function getDebugState(): Promise<unknown> {
  return invoke<unknown>("get_debug_state");
}

export async function triggerDebugPetAction(action: DebugPetAction): Promise<void> {
  return invoke<void>("debug_trigger_pet_action", { action });
}

export function listenDebugLogs(onEntry: (entry: DebugLogEntry) => void): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  void listen<DebugLogEntry>("debug-log", event => {
    if (!disposed) onEntry(event.payload);
  }).then(fn => {
    if (disposed) fn();
    else unlisten = fn;
  }).catch(() => {});
  return () => {
    disposed = true;
    unlisten?.();
  };
}

export function listenDebugPetAction(onAction: (action: DebugPetAction) => void): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  void listen<{ action?: DebugPetAction }>("debug-pet-action", event => {
    if (!disposed && event.payload?.action) onAction(event.payload.action);
  }).then(fn => {
    if (disposed) fn();
    else unlisten = fn;
  }).catch(() => {});
  return () => {
    disposed = true;
    unlisten?.();
  };
}
