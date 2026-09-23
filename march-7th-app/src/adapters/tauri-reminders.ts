import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ReminderCommand, ReminderResponse, ReminderSnapshot } from "../domain/reminder";
import { parseReminderResponse, parseReminderSettings, parseReminderSnapshot } from "./reminder-dto";
export { parseReminderSnapshot } from "./reminder-dto";
export interface ReminderTransport {
  invoke(name: string, args?: Record<string, unknown>): Promise<unknown>;
  listen(name: string, callback: (value: unknown) => void): Promise<() => void>;
}
const nativeTransport: ReminderTransport = { invoke, listen: (name, callback) => listen<unknown>(name, event => callback(event.payload)) };
export type ReminderConnectionError = { stage: "listen" | "snapshot" | "event" | "command"; recovery: "restart" | "retry"; cause: unknown };
// The wire generation orders presentation intents, independently of native backup tickets.
export type SettingsOpenIntent = { generation: number; target: "focus" | "settings"; alreadyVisible: boolean };
export function parseSettingsOpenIntent(value: unknown): SettingsOpenIntent {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw Error("Invalid settings open event");
  const fields = value as Record<string, unknown>;
  if (Object.keys(fields).length !== 3 || !Number.isSafeInteger(fields.generation) || (fields.generation as number) < 1 || !["focus", "settings"].includes(String(fields.target)) || typeof fields.alreadyVisible !== "boolean") throw Error("Invalid settings open event");
  return fields as SettingsOpenIntent;
}
export function connectReminders(options: {
  select(snapshot: ReminderSnapshot): void;
  reportError(error: ReminderConnectionError): void;
  opened?(intent: SettingsOpenIntent): void;
  transport?: ReminderTransport;
}) {
  const transport = options.transport ?? nativeTransport;
  let disposed = false; let failed = false; let connected = false; let latest: ReminderSnapshot | undefined; let lastOpenGeneration = 0;
  const subscriptions: (() => void)[] = [];
  const report = (stage: ReminderConnectionError["stage"], cause: unknown) => { if (!disposed) options.reportError({ stage, recovery: stage === "listen" ? "restart" : "retry", cause }); };
  const accept = (value: unknown) => {
    if (disposed || failed) return undefined;
    const snapshot = parseReminderSnapshot(value);
    if (!latest || snapshot.revision >= latest.revision) { latest = snapshot; options.select(snapshot); }
    return latest;
  };
  const read = async () => {
    if (disposed || !connected) return undefined;
    try { return accept(await transport.invoke("get_reminders")); }
    catch (cause) { report("snapshot", cause); return undefined; }
  };
  void (async () => {
    try {
      const stop = await transport.listen("reminders-changed", value => { if (disposed || failed) return; try { accept(value); } catch (cause) { report("event", cause); } });
      if (disposed) { stop(); return; } subscriptions.push(stop);
      if (options.opened) {
        const stopOpened = await transport.listen("reminder-settings-opened", value => {
          if (disposed) return;
          try {
            const intent = parseSettingsOpenIntent(value);
            if (intent.generation > lastOpenGeneration) { lastOpenGeneration = intent.generation; options.opened?.(intent); }
          } catch (cause) { report("event", cause); }
        });
        if (disposed) { stopOpened(); return; } subscriptions.push(stopOpened);
      }
    } catch (cause) { failed = true; report("listen", cause); subscriptions.splice(0).forEach(stop => stop()); return; }
    connected = true; await read();
  })();
  return {
    refresh: read,
    async command(command: ReminderCommand): Promise<ReminderSnapshot | undefined> {
      if (disposed) return undefined;
      try {
        if (!connected) throw new Error("Reminder subscription is not ready");
        const payload = command.type === "updateSettings" ? { type: command.type, settings: parseReminderSettings(command.settings) } : command;
        const value = await transport.invoke("reminder_command", { command: payload });
        if (disposed) return undefined;
        const reply = parseReminderSnapshot(value);
        accept(reply);
        // Preserve this operation's error/persistence evidence for its caller.
        // Only accept() may deliver state, so an old reply cannot roll back UI.
        return reply;
      } catch (cause) { if (disposed) return undefined; report("command", cause); throw cause; }
    },
    dispose() { if (disposed) return; disposed = true; subscriptions.splice(0).forEach(stop => stop()); },
  };
}
// Response deduplication is separate from snapshot ordering: a state event may
// arrive first, and two distinct responses may themselves arrive out of order.
export function connectReminderResponses(options: { respond(response: ReminderResponse): void; reportError(error: ReminderConnectionError): void; transport?: ReminderTransport }): () => void {
  let disposed = false; let stop: (() => void) | undefined; const seen = new Set<number>();
  void (options.transport ?? nativeTransport).listen("reminder-response", value => {
    if (disposed) return;
    try { const response = parseReminderResponse(value); if (seen.has(response.revision)) return; seen.add(response.revision); options.respond(response); }
    catch (cause) { if (!disposed) options.reportError({ stage: "event", recovery: "retry", cause }); }
  }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }, cause => { if (!disposed) options.reportError({ stage: "listen", recovery: "restart", cause }); });
  return () => { if (disposed) return; disposed = true; stop?.(); seen.clear(); };
}
export function createReminderReady(transport = nativeTransport, token = () => (window as unknown as Record<string, unknown>).__MARCH7_REMINDER_WINDOW_TOKEN__) {
  return async (presentationId: number): Promise<void> => {
    const windowToken = token();
    if (typeof windowToken !== "number" || !Number.isSafeInteger(windowToken) || windowToken < 1 || !Number.isSafeInteger(presentationId) || presentationId < 1) throw new Error("Invalid native reminder window identity");
    await transport.invoke("reminder_ui_ready", { presentationId, windowToken });
  };
}
