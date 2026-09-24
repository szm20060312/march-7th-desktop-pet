import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { documentDouble, ElementDouble } from "../../test/dom-fixture";
import { fresh } from "../../test/reminder-fixture";
import type { ReminderId, ReminderSnapshot } from "../domain/reminder";

const native = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));

const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
function deferred<T>() {
  let resolve!: (value: T) => void; let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}
function presentation(revision: number, id: number, items: ReminderId[]): ReminderSnapshot {
  const snapshot = fresh(revision); snapshot.presentation = { id, mode: "manual", items, closesAt: null };
  snapshot.progress.forEach(progress => { progress.pending = items.includes(progress.id); });
  return snapshot;
}

let dom: ReturnType<typeof documentDouble>;
let windowHost: ElementDouble;
let events: Map<string, (event: { payload: unknown }) => void>;
let stops: Map<string, ReturnType<typeof vi.fn>>;

beforeEach(() => {
  vi.resetModules(); dom = documentDouble(); windowHost = new ElementDouble(); events = new Map(); stops = new Map();
  (windowHost as unknown as Record<string, unknown>).__MARCH7_REMINDER_WINDOW_TOKEN__ = 12;
  native.invoke.mockReset().mockImplementation(async (name: string) => name === "get_reminders" ? fresh() : undefined);
  native.listen.mockReset().mockImplementation(async (name, callback) => {
    events.set(name, callback); const stop = vi.fn(() => events.delete(name)); stops.set(name, stop); return stop;
  });
  vi.stubGlobal("document", dom.document); vi.stubGlobal("window", windowHost); vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => { windowHost.dispatch("pagehide"); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe("real reminder entry / adapter / controller error ownership", () => {
  it("does not show an automatic speech bubble before the selected character is known", async () => {
    const selected = deferred<unknown>(); const initial = presentation(1, 7, ["water"]); initial.presentation!.mode = "automatic";
    native.invoke.mockImplementation((name: string) => name === "get_reminders" ? Promise.resolve(initial)
      : name === "get_selected_character" ? selected.promise : Promise.resolve(undefined));
    await import("./reminder"); await flush();
    expect(native.invoke.mock.calls.some(([name]) => name === "reminder_ui_ready")).toBe(false);
    selected.resolve({ selectedCharacterId: "raiden-shogun", revision: 1, persistence: "saved" }); await flush();
    expect(dom.get("reminder-speaker").textContent).toBe("雷电将军");
    expect(native.invoke.mock.calls.some(([name]) => name === "reminder_ui_ready")).toBe(true);
  });
  it("falls back to an operable neutral reminder if character selection stalls", async () => {
    vi.useFakeTimers();
    try {
      const selected = deferred<unknown>(); const initial = presentation(1, 7, ["water"]); initial.presentation!.mode = "automatic";
      native.invoke.mockImplementation((name: string) => name === "get_reminders" ? Promise.resolve(initial)
        : name === "get_selected_character" ? selected.promise : Promise.resolve(undefined));
      await import("./reminder"); await flush();
      expect(native.invoke.mock.calls.some(([name]) => name === "reminder_ui_ready")).toBe(false);
      vi.advanceTimersByTime(800); await flush();
      expect(dom.get("reminder-speaker").textContent).toBe("桌面伙伴");
      expect(native.invoke.mock.calls.some(([name]) => name === "reminder_ui_ready")).toBe(true);
    } finally { vi.useRealTimers(); }
  });
  it("uses the native selected character for speech and updates without changing the pending action", async () => {
    const initial = presentation(1, 7, ["water"]); initial.presentation!.mode = "automatic";
    native.invoke.mockImplementation(async (name: string) => name === "get_reminders" ? initial
      : name === "get_selected_character" ? { selectedCharacterId: "march-7th", revision: 1, persistence: "saved" } : undefined);
    await import("./reminder"); await flush();
    expect(dom.get("reminder-speaker").textContent).toBe("三月七");
    expect(dom.get("reminder-prompt").textContent).toBe("先喝口水吧，咱们再继续！");
    events.get("selected-character-changed")!({ payload: { selectedCharacterId: "raiden-shogun", revision: 2, persistence: "saved" } }); await flush();
    expect(dom.get("reminder-speaker").textContent).toBe("雷电将军");
    expect(dom.get("reminder-prompt").textContent).toBe("先饮水，再继续手头之事。");
    expect(dom.get("reminder-rows").children[0].children[1].disabled).toBe(false);
  });
  it("does not let an old rejected completion overwrite a newer presentation", async () => {
    const oldCommand = deferred<unknown>(); const initial = presentation(1, 7, ["water"]);
    native.invoke.mockImplementation((name: string) => {
      if (name === "get_reminders") return Promise.resolve(initial);
      if (name === "reminder_command") return oldCommand.promise;
      return Promise.resolve(undefined);
    });
    await import("./reminder"); await flush();
    dom.get("reminder-rows").children[0].children[1].dispatch("click"); await flush();
    events.get("reminders-changed")!({ payload: presentation(3, 8, ["eyes"]) }); await flush();
    expect(dom.get("reminder-rows").children[0].children[0].textContent).toBe("休息眼睛");
    expect(dom.get("reminder-error").textContent).toBe("");
    oldCommand.reject(Error("late operation")); await flush();
    expect(dom.get("reminder-error").textContent).toBe("");
    expect(console.error).toHaveBeenCalledWith("Reminder presentation connection failed", expect.objectContaining({ stage: "command" }));
  });

  it("still displays a current command failure and logs the adapter cause", async () => {
    const currentCommand = deferred<unknown>(); const initial = presentation(1, 7, ["water"]);
    native.invoke.mockImplementation((name: string) => {
      if (name === "get_reminders") return Promise.resolve(initial);
      if (name === "reminder_command") return currentCommand.promise;
      return Promise.resolve(undefined);
    });
    await import("./reminder"); await flush();
    dom.get("reminder-rows").children[0].children[1].dispatch("click"); currentCommand.reject(Error("current operation")); await flush();
    expect(dom.get("reminder-error").textContent).toBe("操作未完成，请重试。");
    expect(console.error).toHaveBeenCalledWith("Reminder presentation connection failed", expect.objectContaining({ stage: "command" }));
  });

  it.each([
    ["listen", "提醒连接未建立，请重启应用。"],
    ["snapshot", "读取或操作失败，请从托盘重新查看提醒。"],
    ["event", "读取或操作失败，请从托盘重新查看提醒。"],
  ] as const)("still displays %s connection errors", async (stage, message) => {
    if (stage === "listen") native.listen.mockRejectedValueOnce(Error("listen"));
    if (stage === "snapshot") native.invoke.mockRejectedValueOnce(Error("snapshot"));
    await import("./reminder"); await flush();
    if (stage === "event") { events.get("reminders-changed")!({ payload: null }); await flush(); }
    expect(dom.get("reminder-error").textContent).toBe(message);
    expect(console.error).toHaveBeenCalledWith("Reminder presentation connection failed", expect.objectContaining({ stage }));
  });

  it("disposes the entry and ignores a late command failure", async () => {
    const pending = deferred<unknown>(); const initial = presentation(1, 7, ["water"]);
    native.invoke.mockImplementation((name: string) => {
      if (name === "get_reminders") return Promise.resolve(initial);
      if (name === "reminder_command") return pending.promise;
      return Promise.resolve(undefined);
    });
    await import("./reminder"); await flush();
    dom.get("reminder-rows").children[0].children[1].dispatch("click"); await flush(); windowHost.dispatch("pagehide");
    pending.reject(Error("after dispose")); await flush();
    expect(stops.get("reminders-changed")).toHaveBeenCalledOnce();
    expect(console.error).not.toHaveBeenCalledWith("Reminder presentation connection failed", expect.objectContaining({ stage: "command" }));
  });
});
