import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { documentDouble, ElementDouble } from "../../test/dom-fixture";
import { fresh } from "../../test/reminder-fixture";

const native = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), hide: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ hide: native.hide }) }));
const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
let dom: ReturnType<typeof documentDouble>;
let windowHost: ElementDouble;
let events: Map<string, (event: { payload: unknown }) => void>;
beforeEach(() => {
  vi.resetModules(); dom = documentDouble(); windowHost = new ElementDouble(); events = new Map();
  native.invoke.mockReset().mockResolvedValue(fresh()); native.hide.mockReset().mockResolvedValue(undefined);
  native.listen.mockReset().mockImplementation(async (name, callback) => { events.set(name, callback); return () => events.delete(name); });
  vi.stubGlobal("document", dom.document); vi.stubGlobal("window", windowHost); vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => { windowHost.dispatch("pagehide"); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
const edit = (minutes: number) => { dom.get("snooze-minutes").value = String(minutes); dom.get("settings-form").dispatch("input"); };
const save = () => dom.get("settings-form").dispatch("submit");

describe("real settings build identity entry", () => {
  const info = { schemaVersion: 1, appVersion: "0.2.0", target: "x86_64-pc-windows-msvc", sourceCommit: "a".repeat(40), sourceState: "modified" };
  it("reads and displays the complete identity without changing reminder drafts", async () => {
    native.invoke.mockImplementation(async name => name === "get_build_info" ? info : fresh());
    await import("./settings"); await flush(); edit(35);
    expect(native.invoke).toHaveBeenCalledWith("get_build_info");
    expect(dom.get("build-commit").textContent).toBe(info.sourceCommit);
    expect(dom.get("build-state").textContent).toContain("本地修改");
    expect(dom.get("snooze-minutes").value).toBe("35");
  });
  it("keeps build identity failures in their own region", async () => {
    native.invoke.mockImplementation(async name => { if (name === "get_build_info") throw Error("identity unavailable"); return fresh(); });
    await import("./settings"); await flush(); edit(35);
    expect(dom.get("build-status").textContent).toContain("版本信息读取失败");
    expect(dom.get("settings-notice").textContent).not.toContain("版本信息");
    expect(dom.get("save-settings").disabled).toBe(false);
  });
  it.each([false, true])("ignores late identity completion after dispose (reject=%s)", async reject => {
    let resolve!: (value: unknown) => void; let fail!: (reason: Error) => void;
    native.invoke.mockImplementation(name => name === "get_build_info" ? new Promise((yes, no) => { resolve = yes; fail = no; }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); windowHost.dispatch("pagehide");
    const prior = dom.get("build-status").textContent;
    if (reject) fail(Error("late")); else resolve(info);
    await flush(); expect(dom.get("build-status").textContent).toBe(prior); expect(dom.get("build-commit").textContent).toBe("");
  });
});

describe("real settings entry / adapter / controller error ownership", () => {
  it.each(["draft", "saved"])("does not let an old rejected save overwrite the reopened %s session", async nextState => {
    await import("./settings"); await flush();
    let rejectOld!: (error: Error) => void;
    native.invoke.mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }));
    edit(30); save(); await flush(); expect(dom.get("save-settings").textContent).toBe("正在保存…");
    dom.get("close-settings").dispatch("click"); await flush(); expect(native.invoke).toHaveBeenCalledWith("hide_reminder_settings");
    native.invoke.mockResolvedValueOnce(fresh(3)); events.get("reminder-settings-opened")!({ payload: null }); await flush(); edit(50);
    if (nextState === "saved") { const saved = fresh(4); saved.settings.snoozeMinutes = 50; native.invoke.mockResolvedValueOnce(saved); save(); await flush(); expect(dom.get("settings-notice").textContent).toBe("设置已保存。"); }
    const notice = dom.get("settings-notice").textContent; const status = dom.get("settings-status").textContent;
    rejectOld(Error("late operation")); await flush();
    expect(dom.get("settings-notice").textContent).toBe(notice); expect(dom.get("settings-status").textContent).toBe(status); expect(dom.get("snooze-minutes").value).toBe("50");
    expect(console.error).toHaveBeenCalledWith("Reminder settings connection failed", expect.objectContaining({ stage: "command" }));
  });
  it("still displays a current save error, retains the draft, and logs the adapter cause", async () => {
    await import("./settings"); await flush(); native.invoke.mockRejectedValueOnce(Error("current save")); edit(40); save(); await flush();
    expect(dom.get("settings-notice").textContent).toBe("保存失败，请重试；当前草稿仍保留。"); expect(dom.get("snooze-minutes").value).toBe("40"); expect(dom.get("save-settings").disabled).toBe(false);
    expect(console.error).toHaveBeenCalledWith("Reminder settings connection failed", expect.objectContaining({ stage: "command" }));
  });
  it("still displays connection read errors outside command sessions", async () => {
    native.invoke.mockImplementation(async name => { if (name === "get_reminders") throw Error("read"); return fresh(); }); await import("./settings"); await flush(); expect(dom.get("settings-notice").textContent).toContain("读取或操作失败");
  });
});
