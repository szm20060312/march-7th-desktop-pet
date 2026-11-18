import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SelectedCharacterSnapshot } from "../application/character-presentation";
export type CharacterSnapshot = SelectedCharacterSnapshot & { persistence: "default" | "saved" | "fallback" | "sessionOnly" | "saveFailed" };
export type CharacterConnectionError =
  | { stage: "listen"; recovery: "restart"; cause: unknown }
  | { stage: "snapshot" | "event"; recovery: "tray"; cause: unknown };
export function parseCharacterSnapshot(value: unknown, ids: readonly string[]): CharacterSnapshot {
  if (!value || typeof value !== "object") throw new Error("Invalid character snapshot");
  const raw = value as Record<string, unknown>;
  if (typeof raw.selectedCharacterId !== "string" || !ids.includes(raw.selectedCharacterId)
    || typeof raw.revision !== "number" || !Number.isSafeInteger(raw.revision) || raw.revision < 0
    || !["default", "saved", "fallback", "sessionOnly", "saveFailed"].includes(raw.persistence as string)) throw new Error("Invalid character snapshot fields");
  return { characterId: raw.selectedCharacterId, revision: raw.revision, persistence: raw.persistence as CharacterSnapshot["persistence"] };
}
// Register before reading: an event can beat the read reply, so revisions are authoritative.
export function connectCharacterSelection(options: {
  ids: readonly string[];
  select(snapshot: CharacterSnapshot): void;
  reportError(error: CharacterConnectionError): void;
  listen?: (callback: (value: unknown) => void) => Promise<() => void>;
  get?: () => Promise<unknown>;
}): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  let revision = -1;
  const accept = (value: unknown, stage: "snapshot" | "event") => {
    if (disposed) return;
    try {
      const snapshot = parseCharacterSnapshot(value, options.ids);
      if (snapshot.revision < revision) return;
      revision = snapshot.revision;
      options.select(snapshot);
    } catch (cause) { if (!disposed) options.reportError({ stage, recovery: "tray", cause }); }
  };
  const subscribe = options.listen ?? (callback => listen<unknown>("selected-character-changed", event => callback(event.payload)));
  void (async () => {
    let stop: () => void;
    try {
      stop = await subscribe(value => accept(value, "event"));
    } catch (cause) {
      if (!disposed) options.reportError({ stage: "listen", recovery: "restart", cause });
      return;
    }
    if (disposed) { stop(); return; }
    unlisten = stop;
    try {
      accept(await (options.get ?? (() => invoke<unknown>("get_selected_character")))(), "snapshot");
    } catch (cause) {
      if (!disposed) options.reportError({ stage: "snapshot", recovery: "tray", cause });
    }
  })();
  return () => { if (disposed) return; disposed = true; unlisten?.(); unlisten = undefined; };
}
