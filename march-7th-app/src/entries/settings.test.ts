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
    native.invoke.mockResolvedValueOnce(fresh(3)); events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "settings" } }); await flush(); edit(50);
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

describe("local backup entry stays separate from reminder drafts", () => {
  const preview = { ticket: 7, preview: { createdAtUtcMs: 100, selectedCharacterId: "raiden-shogun", hasDesktopPlacement: true, focus: { status: "paused", defaultedFromV1: false }, reminders: { items: [{ id: "water", enabled: true, intervalMinutes: 60 }, { id: "move", enabled: false, intervalMinutes: 45 }, { id: "eyes", enabled: true, intervalMinutes: 30 }], activeHours: { kind: "daily", start: 540, end: 1320 }, snoozeMinutes: 10, pendingCount: 2, paused: true, quietUntilUtcMs: null, snoozePending: false } } };
  it("previews limited coverage, confirms only on explicit action, and preserves reminder draft", async () => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.resolve(preview) : name === "confirm_local_backup" ? Promise.resolve({ transactionId: "abc", restartRequired: true }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); edit(37);
    dom.get("import-backup").dispatch("click"); await flush();
    expect(dom.get("backup-preview").hidden).toBe(false);
    expect(dom.get("import-character").textContent).toContain("雷电将军");
    expect(dom.get("import-pending").textContent).toContain("待处理 2 项");
    expect(dom.get("import-focus").textContent).toBe("已暂停");
    expect(native.invoke.mock.calls.some(([name]) => name === "confirm_local_backup")).toBe(false);
    dom.get("confirm-import").dispatch("click"); await flush();
    expect(native.invoke).toHaveBeenCalledWith("confirm_local_backup", { ticket: 7 });
    expect(dom.get("backup-status").textContent).toContain("下次启动");
    expect(dom.get("snooze-minutes").value).toBe("37");
  });
  it("explains a v1 backup will restore an empty focus session without private fields", async () => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.resolve({ ...preview, preview: { ...preview.preview, focus: { status: "idle", defaultedFromV1: true } } }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("import-backup").dispatch("click"); await flush();
    expect(dom.get("import-focus").textContent).toContain("旧版 v1");
    expect(dom.get("import-focus").textContent).toContain("空专注会话");
    expect(dom.get("import-focus").textContent).not.toContain("anchor");
  });
  it("rejects a preview that leaks internal focus clock fields", async () => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.resolve({ ...preview, preview: { ...preview.preview, focus: { status: "running", defaultedFromV1: false, anchor_utc_ms: 100 } } }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("import-backup").dispatch("click"); await flush();
    expect(dom.get("backup-preview").hidden).toBe(true);
    expect(dom.get("backup-status").textContent).toContain("无法预览");
  });
  it("invalidates late selection after close and keeps export and import mutually disabled", async () => {
    let finish!: (value: unknown) => void;
    native.invoke.mockImplementation(name => name === "select_local_backup" ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("import-backup").dispatch("click"); await flush();
    expect(dom.get("export-backup").disabled).toBe(true);
    dom.get("close-settings").dispatch("click"); await flush();
    events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "settings" } }); await flush();
    finish(preview); await flush();
    expect(dom.get("backup-preview").hidden).toBe(true);
    expect(dom.get("confirm-import").disabled).toBe(true);
  });
  it("cancels a preview and never confirms, then accepts a fresh selection", async () => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.resolve(preview) : name === "cancel_local_backup" ? Promise.resolve(undefined) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("import-backup").dispatch("click"); await flush();
    dom.get("cancel-import").dispatch("click"); await flush();
    expect(native.invoke).toHaveBeenCalledWith("cancel_local_backup", { ticket: 7 });
    expect(native.invoke.mock.calls.some(([name]) => name === "confirm_local_backup")).toBe(false);
    expect(dom.get("backup-preview").hidden).toBe(true);
    dom.get("import-backup").dispatch("click"); await flush();
    expect(native.invoke.mock.calls.filter(([name]) => name === "select_local_backup")).toHaveLength(2);
  });
  it.each([
    ["backupTooLarge", "2 MiB"], ["backupUnsupportedVersion", "更新"],
    ["backupChecksumMismatch", "校验失败"], ["pendingImportExists", "已有待导入"],
    ["dataSetWriteFailed", "原数据未改变"],
  ])("shows bounded import error %s without disturbing drafts", async (code, expected) => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.resolve(preview) : name === "confirm_local_backup" ? Promise.reject({ code }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); edit(38);
    dom.get("import-backup").dispatch("click"); await flush();
    dom.get("confirm-import").dispatch("click"); await flush();
    expect(dom.get("backup-status").textContent).toContain(expected);
    expect(dom.get("snooze-minutes").value).toBe("38");
    expect(dom.get("settings-notice").textContent).toBe("");
  });
  it("rejects a malformed native preview instead of showing or confirming it", async () => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.resolve({ ticket: 7, preview: { reminders: { items: [] } } }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("import-backup").dispatch("click"); await flush();
    expect(dom.get("backup-preview").hidden).toBe(true);
    expect(dom.get("backup-status").textContent).toContain("无法预览");
    dom.get("confirm-import").dispatch("click"); await flush();
    expect(native.invoke.mock.calls.some(([name]) => name === "confirm_local_backup")).toBe(false);
  });
  it("shows an existing pending import before opening a new preview", async () => {
    native.invoke.mockImplementation(name => name === "select_local_backup" ? Promise.reject({ code: "pendingImportExists" }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("import-backup").dispatch("click"); await flush();
    expect(dom.get("backup-status").textContent).toContain("先正常退出并重新打开");
    expect(dom.get("backup-preview").hidden).toBe(true);
  });
  it("disables repeat export and preserves an unsaved reminder draft", async () => {
    let finish!: (value: unknown) => void;
    native.invoke.mockImplementation(name => name === "export_local_backup" ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); edit(37);
    dom.get("export-backup").dispatch("click"); dom.get("export-backup").dispatch("click"); await flush();
    expect(native.invoke.mock.calls.filter(([name]) => name === "export_local_backup")).toHaveLength(1);
    expect(dom.get("export-backup").disabled).toBe(true);
    finish("saved"); await flush();
    expect(dom.get("backup-status").textContent).toContain("保存成功");
    expect(dom.get("snooze-minutes").value).toBe("37");
    expect(dom.get("settings-notice").textContent).toBe("");
  });
  it("keeps old dialog responses out of a reopened window", async () => {
    let finish!: (value: unknown) => void;
    native.invoke.mockImplementation(name => name === "export_local_backup" ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("export-backup").dispatch("click"); await flush();
    dom.get("close-settings").dispatch("click"); await flush();
    events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "settings" } }); await flush();
    const before = dom.get("backup-status").textContent;
    finish("saved"); await flush();
    expect(dom.get("backup-status").textContent).toBe(before);
    expect(dom.get("export-backup").disabled).toBe(false);
  });
  it("distinguishes cancellation and existing file without touching reminder status", async () => {
    native.invoke.mockImplementation(name => name === "export_local_backup" ? Promise.resolve("cancelled") : Promise.resolve(fresh()));
    await import("./settings"); await flush(); dom.get("export-backup").dispatch("click"); await flush();
    expect(dom.get("backup-status").textContent).toContain("已取消");
    native.invoke.mockImplementation(name => name === "export_local_backup" ? Promise.reject({ code: "backupAlreadyExists" }) : Promise.resolve(fresh()));
    dom.get("export-backup").dispatch("click"); await flush();
    expect(dom.get("backup-status").textContent).toContain("已存在");
    expect(dom.get("settings-notice").textContent).toBe("");
  });
});

describe("focus settings entry uses the committed Rust state", () => {
  const focus = (revision: number, session: object) => ({ snapshot: { revision, data: { version: 1, session }, error: null, stopped: false }, completedNow: false, error: null });
  it("ignores an initial focus read failure after close and a successful reopened read", async () => {
    let failOld!: (reason: unknown) => void;
    const current = focus(3, { status: "idle" });
    native.invoke.mockImplementation(name => name === "get_focus" ? new Promise((yes, no) => {
      if (!failOld) failOld = no; else yes(current);
    }) : Promise.resolve(fresh()));
    await import("./settings"); await flush();
    dom.get("close-settings").dispatch("click"); await flush();
    events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "focus" } }); await flush();
    expect(dom.get("focus-state").textContent).toBe("尚未开始");
    failOld(Error("old read")); await flush();
    expect(dom.get("focus-notice").textContent).toBe("");
  });
  it("finds focus from a newly opened window and again from an already open window, ignoring stale intent", async () => {
    const scroll = vi.fn(); (dom.get("focus-section") as ElementDouble & { scrollIntoView: typeof scroll }).scrollIntoView = scroll;
    await import("./settings"); await flush();
    events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "focus" } }); await flush();
    expect(scroll).toHaveBeenCalledWith({ block: "start" });
    scroll.mockClear();
    events.get("reminder-settings-opened")!({ payload: { generation: 2, target: "settings" } }); await flush();
    expect(scroll).not.toHaveBeenCalled();
    events.get("reminder-settings-opened")!({ payload: { generation: 3, target: "focus" } }); await flush();
    expect(scroll).toHaveBeenCalledTimes(1);
    events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "focus" } }); await flush();
    expect(scroll).toHaveBeenCalledTimes(1);
  });
  it("starts, pauses, resumes, ends early, and never invokes mutating view for display", async () => {
    let current = focus(1, { status: "idle" });
    native.invoke.mockImplementation(async (name, args) => {
      if (name === "get_focus") return current;
      if (name === "focus_command") {
        const type = args.command.type;
        current = focus(current.snapshot.revision + 1, type === "start" || type === "resume"
          ? { status: "running", duration_ms: 1_500_000, remaining_ms: 1_500_000, anchor_utc_ms: Date.now() }
          : type === "pause" ? { status: "paused", duration_ms: 1_500_000, remaining_ms: 800_000 }
            : { status: "finished", duration_ms: 1_500_000, outcome: "endedEarly", feedback: "none" });
        return current.snapshot;
      }
      return fresh();
    });
    await import("./settings"); await flush();
    expect(dom.get("focus-start").disabled).toBe(false);
    dom.get("focus-start").dispatch("click"); await flush();
    expect(native.invoke).toHaveBeenCalledWith("focus_command", { command: { type: "start", durationMs: 1_500_000 } });
    expect(dom.get("focus-state").textContent).toBe("专注进行中");
    dom.get("focus-pause").dispatch("click"); await flush();
    expect(dom.get("focus-state").textContent).toBe("已暂停");
    dom.get("focus-resume").dispatch("click"); await flush();
    dom.get("focus-end").dispatch("click"); await flush();
    expect(dom.get("focus-state").textContent).toBe("已提前结束");
    expect(native.invoke.mock.calls.some(([name, args]) => name === "focus_command" && args.command.type === "view")).toBe(false);
  });

  it("keeps failed and late commands from reporting success after close and reopen", async () => {
    let reject!: (reason: unknown) => void;
    const initial = focus(1, { status: "running", duration_ms: 1_500_000, remaining_ms: 1_000_000, anchor_utc_ms: 100 });
    native.invoke.mockImplementation((name) => name === "get_focus" ? Promise.resolve(initial) : name === "focus_command" ? new Promise((_, no) => { reject = no; }) : Promise.resolve(fresh()));
    await import("./settings"); await flush();
    dom.get("focus-pause").dispatch("click"); await flush();
    dom.get("close-settings").dispatch("click"); await flush();
    events.get("reminder-settings-opened")!({ payload: { generation: 1, target: "settings" } }); await flush();
    const newer = focus(2, { status: "paused", duration_ms: 1_500_000, remaining_ms: 900_000 });
    events.get("focus-changed")!({ payload: newer }); await flush();
    reject({ code: "writeFailed" }); await flush();
    expect(dom.get("focus-state").textContent).toBe("已暂停");
    expect(dom.get("focus-notice").textContent).toBe("");
  });
});
