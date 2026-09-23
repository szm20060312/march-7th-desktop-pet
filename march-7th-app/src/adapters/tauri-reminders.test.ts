import { describe, expect, it, vi } from "vitest";
import { connectReminders, connectReminderPrompts, connectReminderResponses, createReminderReady, parseReminderPromptEvent, parseReminderSnapshot, parseSettingsOpenIntent, type ReminderTransport } from "./tauri-reminders";

export function snapshot(revision = 1) {
  return { revision, settings: { items: ["water", "move", "eyes"].map(id => ({ id, enabled: false, intervalMinutes: 60 })), activeHours: { kind: "allDay" }, snoozeMinutes: 10 }, progress: ["water", "move", "eyes"].map(id => ({ id, nextDueAt: null, pending: false, autoHandled: false })), paused: false, quiet: null, snoozePending: false, presentation: null, persistence: { status: "saved", code: null }, runtimeError: null, stopped: false };
}
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function deferred<T>() { let resolve!: (v: T) => void; let reject!: (e: unknown) => void; const promise = new Promise<T>((r, j) => { resolve = r; reject = j; }); return { promise, resolve, reject }; }
function host() {
  const events = new Map<string, (v: unknown) => void>(); const stop = vi.fn();
  const invoke = vi.fn().mockResolvedValue(snapshot());
  const listen = vi.fn(async (name: string, cb: (v: unknown) => void): Promise<() => void> => { events.set(name, cb); return stop; });
  return { events, stop, invoke, listen };
}
describe("reminder native boundary", () => {
  it("accepts only targeted settings-open generations and ignores delayed older opens", async () => {
    expect(parseSettingsOpenIntent({ generation: 4, target: "focus", alreadyVisible: true })).toEqual({ generation: 4, target: "focus", alreadyVisible: true });
    for (const bad of [null, { generation: 0, target: "focus", alreadyVisible: false }, { generation: 1, target: "other", alreadyVisible: false }, { generation: 1, target: "focus", path: "private" }, { generation: 1, target: "focus", alreadyVisible: "yes" }]) expect(() => parseSettingsOpenIntent(bad)).toThrow();
    const h = host(); const opened = vi.fn(); const c = connectReminders({ select: vi.fn(), opened, reportError: vi.fn(), transport: h }); await flush();
    h.events.get("reminder-settings-opened")!({ generation: 1, target: "focus", alreadyVisible: false });
    h.events.get("reminder-settings-opened")!({ generation: 2, target: "settings", alreadyVisible: true });
    h.events.get("reminder-settings-opened")!({ generation: 1, target: "focus", alreadyVisible: false });
    expect(opened.mock.calls.map(([intent]) => intent)).toEqual([{ generation: 1, target: "focus", alreadyVisible: false }, { generation: 2, target: "settings", alreadyVisible: true }]);
    c.dispose();
  });
  it("validates required fields, complete unique identities and scalar bounds; canonicalizes ID order", () => {
    const raw = snapshot(); raw.settings.items.reverse(); raw.progress.reverse();
    expect(parseReminderSnapshot(raw).settings.items.map(i => i.id)).toEqual(["water", "move", "eyes"]);
    for (const bad of [null, {}, { ...raw, revision: Infinity }, { ...raw, paused: 1 }, { ...raw, runtimeError: 5 }, { ...raw, persistence: { status: "unknown", code: null } }, { ...raw, progress: [raw.progress[0], raw.progress[0], raw.progress[2]] }, { ...raw, presentation: { id: 0, mode: "manual", items: [], closesAt: null } }, { ...raw, quiet: { until: -1, durationMinutes: 10 } }, { ...raw, settings: { ...raw.settings, snoozeMinutes: 121 } }, { ...raw, stopped: undefined }]) expect(() => parseReminderSnapshot(bad)).toThrow();
  });
  it("subscribes before reading and fences events/get/command replies together", async () => {
    const h = host(); const first = deferred<unknown>(); const reply = deferred<unknown>(); h.invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(reply.promise);
    const select = vi.fn(); const c = connectReminders({ select, reportError: vi.fn(), transport: h });
    expect(h.invoke).not.toHaveBeenCalled(); await flush();
    h.events.get("reminders-changed")!(snapshot(4)); first.resolve(snapshot(1)); await flush();
    const result = c.command({ type: "setPaused", paused: true }); h.events.get("reminders-changed")!(snapshot(6)); reply.resolve(snapshot(5));
    expect((await result)?.revision).toBe(5); expect(select.mock.calls.map(v => v[0].revision)).toEqual([4, 6]); c.dispose();
  });
  it("sends exact Rust command envelopes, including field-free allDay", async () => {
    const h = host(); const c = connectReminders({ select: vi.fn(), reportError: vi.fn(), transport: h }); await flush();
    const settings = parseReminderSnapshot(snapshot()).settings;
    await c.command({ type: "updateSettings", settings }); await c.command({ type: "complete", id: "eyes" }); await c.command({ type: "snoozeAll" }); await c.command({ type: "dismiss", presentationId: 9 }); await c.command({ type: "showPending" });
    expect(h.invoke.mock.calls.slice(1)).toEqual([
      ["reminder_command", { command: { type: "updateSettings", settings } }], ["reminder_command", { command: { type: "complete", id: "eyes" } }], ["reminder_command", { command: { type: "snoozeAll" } }], ["reminder_command", { command: { type: "dismiss", presentationId: 9 } }], ["reminder_command", { command: { type: "showPending" } }],
    ]); expect(settings.activeHours).toEqual({ kind: "allDay" }); c.dispose();
  });
  it("distinguishes fatal listen failure from recoverable reads and operations", async () => {
    const h = host(); const error = vi.fn(); h.listen.mockRejectedValueOnce(Error("no listener"));
    connectReminders({ select: vi.fn(), reportError: error, transport: h }); await flush(); expect(error.mock.calls[0][0].recovery).toBe("restart"); expect(h.invoke).not.toHaveBeenCalled();
    h.invoke.mockRejectedValue(Error("read")); const c = connectReminders({ select: vi.fn(), reportError: error, transport: h }); await flush(); expect(error.mock.calls[error.mock.calls.length - 1][0].recovery).toBe("retry");
    await expect(c.command({ type: "showPending" })).rejects.toThrow(); c.dispose();
  });
  it.each(["resolve", "reject"])("discards late command %s after disposal", async outcome => {
    const h = host(); const error = vi.fn(); const select = vi.fn(); const c = connectReminders({ select, reportError: error, transport: h }); await flush(); select.mockClear();
    const pending = deferred<unknown>(); h.invoke.mockReturnValue(pending.promise); const reply = c.command({ type: "showPending" }); c.dispose();
    if (outcome === "resolve") pending.resolve(snapshot(9)); else pending.reject(Error("late"));
    expect(await reply).toBeUndefined(); expect(select).not.toHaveBeenCalled(); expect(error).not.toHaveBeenCalled();
  });
  it("cleans a late subscription and never starts its read", async () => {
    const h = host(); const pending = deferred<() => void>(); h.listen.mockReturnValue(pending.promise); const c = connectReminders({ select: vi.fn(), reportError: vi.fn(), transport: h }); c.dispose(); pending.resolve(h.stop); await flush(); expect(h.stop).toHaveBeenCalledOnce(); expect(h.invoke).not.toHaveBeenCalled();
  });
  it.each(["resolve", "reject"])("discards a late initial read %s after disposal", async outcome => {
    const h = host(); const pending = deferred<unknown>(); h.invoke.mockReturnValue(pending.promise); const select = vi.fn(); const error = vi.fn();
    const c = connectReminders({ select, reportError: error, transport: h }); await flush(); c.dispose();
    if (outcome === "resolve") pending.resolve(snapshot(9)); else pending.reject(Error("late read")); await flush(); expect(select).not.toHaveBeenCalled(); expect(error).not.toHaveBeenCalled();
  });
  it("stops partial subscriptions if the settings-open listener fails, ignoring queued events", async () => {
    const h = host(); const select = vi.fn(); h.listen.mockImplementationOnce(async (name, callback) => { h.events.set(name, callback); return h.stop; }).mockRejectedValueOnce(Error("opened listener"));
    const c = connectReminders({ select, opened: vi.fn(), reportError: vi.fn(), transport: h }); await flush();
    h.events.get("reminders-changed")!(snapshot(5)); expect(select).not.toHaveBeenCalled(); expect(h.stop).toHaveBeenCalledOnce(); expect(h.invoke).not.toHaveBeenCalled(); c.dispose();
  });
  it("deduplicates responses independently from snapshot revisions and ignores disposed events", async () => {
    const h = host(); const respond = vi.fn(); const dispose = connectReminderResponses({ respond, reportError: vi.fn(), transport: h }); await flush();
    h.events.get("reminder-response")!({ revision: 4, type: "complete", id: "water" }); h.events.get("reminder-response")!({ revision: 4, type: "complete", id: "water" }); h.events.get("reminder-response")!({ revision: 6, type: "snoozeAll" }); h.events.get("reminder-response")!({ revision: 5, type: "complete", id: "eyes" });
    expect(respond).toHaveBeenCalledTimes(3); dispose(); h.events.get("reminder-response")!({ revision: 8, type: "snoozeAll" }); expect(respond).toHaveBeenCalledTimes(3);
  });
  it("accepts only live, unique automatic presentation prompts with known items", async () => {
    for (const bad of [null, { presentationId: 0, items: ["water"] }, { presentationId: 1, items: [] }, { presentationId: 1, items: ["water", "water"] }, { presentationId: 1, items: ["other"] }, { presentationId: 1, items: ["water"], path: "private" }]) {
      expect(() => parseReminderPromptEvent(bad)).toThrow();
    }
    const h = host(); const remind = vi.fn(); const reportError = vi.fn();
    const dispose = connectReminderPrompts({ remind, reportError, transport: h }); await flush();
    h.events.get("reminder-prompt")!({ presentationId: 3, items: ["water"] });
    h.events.get("reminder-prompt")!({ presentationId: 3, items: ["water"] });
    h.events.get("reminder-prompt")!({ presentationId: 2, items: ["move"] });
    h.events.get("reminder-prompt")!({ presentationId: 4, items: ["eyes", "move"] });
    expect(remind.mock.calls.map(([event]) => event.presentationId)).toEqual([3, 4]);
    expect(reportError).not.toHaveBeenCalled(); expect(h.invoke).not.toHaveBeenCalled();
    dispose(); h.events.get("reminder-prompt")!({ presentationId: 5, items: ["water"] });
    expect(remind).toHaveBeenCalledTimes(2);
  });
  it("uses only a valid native window token for the presentation handshake", async () => {
    const h: ReminderTransport = host(); const ready = createReminderReady(h, () => 12); await ready(7); expect(h.invoke).toHaveBeenCalledWith("reminder_ui_ready", { presentationId: 7, windowToken: 12 });
    await expect(createReminderReady(h, () => undefined)(7)).rejects.toThrow(); await expect(createReminderReady(h, () => 1.5)(7)).rejects.toThrow();
  });
  it("cleans a late response listener and does not replay pre-disposal responses", async () => {
    const h = host(); const pending = deferred<() => void>(); h.listen.mockReturnValue(pending.promise); const respond = vi.fn(); const error = vi.fn();
    const dispose = connectReminderResponses({ respond, reportError: error, transport: h }); dispose(); pending.resolve(h.stop); await flush(); expect(h.stop).toHaveBeenCalledOnce(); expect(respond).not.toHaveBeenCalled(); expect(error).not.toHaveBeenCalled();
  });
});
