import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { FocusChange, FocusCommand, FocusSession, FocusSnapshot } from "../domain/focus";

export interface FocusTransport {
  invoke(name: string, args?: Record<string, unknown>): Promise<unknown>;
  listen(name: string, callback: (value: unknown) => void): Promise<() => void>;
}
const nativeTransport: FocusTransport = { invoke, listen: (name, callback) => listen<unknown>(name, event => callback(event.payload)) };
const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const safe = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) >= 0;
const duration = (value: unknown): value is number => safe(value) && value >= 60_000 && value <= 14_400_000;
const remaining = (value: unknown, total: number): value is number => safe(value) && value > 0 && value <= total;
const code = (value: unknown): value is string | null => value === null || typeof value === "string";

function parseSession(value: unknown): FocusSession {
  if (!record(value)) throw Error("Invalid focus session");
  const keys = Object.keys(value);
  const exact = (...fields: string[]) => keys.length === fields.length && fields.every(field => field in value);
  switch (value.status) {
    case "idle": if (exact("status")) return { status: "idle" }; break;
    case "running": if (exact("status", "duration_ms", "remaining_ms", "anchor_utc_ms") && duration(value.duration_ms) && remaining(value.remaining_ms, value.duration_ms) && safe(value.anchor_utc_ms)) return value as FocusSession; break;
    case "paused": case "interrupted": if (exact("status", "duration_ms", "remaining_ms") && duration(value.duration_ms) && remaining(value.remaining_ms, value.duration_ms)) return value as FocusSession; break;
    case "finished": if (exact("status", "duration_ms", "outcome", "feedback") && duration(value.duration_ms) && ["natural", "endedEarly", "abandoned"].includes(String(value.outcome)) && ["none", "pending", "dismissed"].includes(String(value.feedback)) && (value.outcome === "natural") === (value.feedback !== "none")) return value as FocusSession; break;
  }
  throw Error("Invalid focus session");
}

export function parseFocusSnapshot(value: unknown): FocusSnapshot {
  if (!record(value) || !safe(value.revision) || !code(value.error) || typeof value.stopped !== "boolean") throw Error("Invalid focus snapshot");
  if (value.data === null) return value as FocusSnapshot;
  if (!record(value.data) || value.data.version !== 1) throw Error("Invalid focus data");
  return { revision: value.revision, data: { version: 1, session: parseSession(value.data.session) }, error: value.error, stopped: value.stopped };
}
export function parseFocusChange(value: unknown): FocusChange {
  if (!record(value) || typeof value.completedNow !== "boolean" || !code(value.error)) throw Error("Invalid focus change");
  return { snapshot: parseFocusSnapshot(value.snapshot), completedNow: value.completedNow, error: value.error };
}

export function connectFocus(options: { select(change: FocusChange): void; reportError(error: { stage: "listen" | "snapshot" | "event" | "command"; cause: unknown }): void; transport?: FocusTransport }) {
  const transport = options.transport ?? nativeTransport;
  let disposed = false; let ready = false; let failed = false; let lastRevision = -1; let eventEpoch = 0;
  let stop: (() => void) | undefined;
  const report = (stage: "listen" | "snapshot" | "event" | "command", cause: unknown) => { if (!disposed) options.reportError({ stage, cause }); };
  const accept = (value: unknown) => {
    const change = parseFocusChange(value);
    if (!disposed && change.snapshot.revision >= lastRevision) {
      lastRevision = change.snapshot.revision;
      options.select(change);
    }
  };
  const refresh = async () => {
    if (disposed || !ready) return;
    const startedAt = eventEpoch;
    try {
      const change = parseFocusChange(await transport.invoke("get_focus"));
      if (startedAt !== eventEpoch && change.snapshot.revision <= lastRevision) return;
      accept(change);
    }
    catch (cause) { report("snapshot", cause); }
  };
  void (async () => {
    try {
      const unsubscribe = await transport.listen("focus-changed", value => {
        if (disposed || failed) return;
        try {
          eventEpoch++;
          const change = parseFocusChange(value);
          // A queued error can arrive after a newer window generation or a
          // successful commit. Read the current diagnostic before displaying it.
          if (change.error) void refresh();
          else accept(change);
        } catch (cause) { report("event", cause); }
      });
      if (disposed) { unsubscribe(); return; }
      stop = unsubscribe; ready = true; await refresh();
    } catch (cause) { failed = true; report("listen", cause); }
  })();
  return {
    refresh,
    async command(command: FocusCommand): Promise<FocusSnapshot> {
      if (disposed || !ready || failed) throw Error("Focus connection unavailable");
      try {
        const snapshot = parseFocusSnapshot(await transport.invoke("focus_command", { command }));
        if (snapshot.revision >= lastRevision) { lastRevision = snapshot.revision; options.select({ snapshot, completedNow: false, error: null }); }
        return snapshot;
      } catch (cause) { report("command", cause); throw cause; }
    },
    dispose() { disposed = true; stop?.(); },
  };
}
