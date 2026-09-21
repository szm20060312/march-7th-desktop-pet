import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SelectedCharacterSnapshot } from "../application/character-presentation";
export type CharacterSnapshot = SelectedCharacterSnapshot & { persistence: "default" | "saved" | "fallback" | "sessionOnly" | "saveFailed" };
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
  reportError(error: unknown): void;
  listen?: (callback: (value: unknown) => void) => Promise<() => void>;
  get?: () => Promise<unknown>;
}): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  let revision = -1;
  const accept = (value: unknown) => {
    if (disposed) return;
    try {
      const snapshot = parseCharacterSnapshot(value, options.ids);
      if (snapshot.revision < revision) return;
      revision = snapshot.revision;
      options.select(snapshot);
    } catch (error) { options.reportError(error); }
  };
  const subscribe = options.listen ?? (callback => listen<unknown>("selected-character-changed", event => callback(event.payload)));
  void (async () => {
    try {
      const stop = await subscribe(accept);
      if (disposed) { stop(); return; }
      unlisten = stop;
      accept(await (options.get ?? (() => invoke<unknown>("get_selected_character")))());
    } catch (error) { if (!disposed) options.reportError(error); }
  })();
  return () => { if (disposed) return; disposed = true; unlisten?.(); unlisten = undefined; };
}
