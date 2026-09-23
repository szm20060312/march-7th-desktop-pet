import { describe, expect, it, vi } from "vitest";
import { createReminderSettingsController, type SettingsViewState } from "./reminder-settings";
import { fresh } from "../../test/reminder-fixture";
import type { ReminderSnapshot } from "../domain/reminder";
function setup() {
  let state!: SettingsViewState; const command = vi.fn().mockResolvedValue(fresh(2)); const close = vi.fn();
  const c = createReminderSettingsController({ render: v => { state = v; }, command, close });
  c.receive(fresh());
  return { c, command, close, state: () => state };
}
describe("reminder settings draft", () => {
  it("preserves edited fields across snapshots and immediate pause commands", async () => {
    const h = setup(); const draft = h.state().draft!; draft.items[0].enabled = true; draft.snoozeMinutes = 20; h.c.edit(draft);
    const incoming = fresh(5); incoming.paused = true; h.c.receive(incoming); h.c.receive(fresh(3));
    expect(h.state().draft?.snoozeMinutes).toBe(20); expect(h.state().snapshot?.paused).toBe(true);
    await h.c.togglePaused(); expect(h.command).toHaveBeenCalledWith({ type: "setPaused", paused: false }); expect(h.state().dirty).toBe(true);
  });
  it.each(["loading", "readOnly", "stopped"])("locks configuration in %s state", async status => {
    const h = setup(); const s = fresh(2); if (status === "stopped") s.stopped = true; else s.persistence.status = status as "loading" | "readOnly";
    h.c.receive(s); const draft = fresh().settings; draft.items[0].enabled = true; h.c.edit(draft); await h.c.save();
    expect(h.state().editable).toBe(false); expect(h.state().draft?.items[0].enabled).toBe(false); expect(h.command).not.toHaveBeenCalled();
  });
  it.each(["saved", "unsaved", "clock", "mismatch"])("reports %s truthfully and clears draft only on confirmed saved settings", async result => {
    const h = setup(); const draft = fresh().settings; draft.items[0].enabled = true; h.c.edit(draft);
    const reply = fresh(3); if (result !== "mismatch") reply.settings = draft;
    if (result === "unsaved") reply.persistence.status = "unsaved";
    if (result === "clock") reply.runtimeError = "invalidTime";
    h.command.mockResolvedValue(reply); await h.c.save();
    expect(h.state().dirty).toBe(result !== "saved");
    expect(h.state().notice).toContain(({ saved: "已保存", unsaved: "未保存", clock: "未能", mismatch: "未能" })[result]);
  });
  it("prevents duplicate submission and preserves a draft when a newer snapshot overtakes a reply", async () => {
    const h = setup(); const draft = fresh().settings; draft.items[0].enabled = true; h.c.edit(draft);
    let resolve!: (s: ReminderSnapshot) => void; h.command.mockReturnValue(new Promise<ReminderSnapshot>(r => { resolve = r; }));
    const save = h.c.save(); await h.c.save(); expect(h.command).toHaveBeenCalledOnce(); expect(h.state().saving).toBe(true);
    h.c.receive(fresh(9)); const reply = fresh(8); reply.settings = draft; resolve(reply); await save;
    expect(h.state().dirty).toBe(true); expect(h.state().notice).not.toContain("已保存");
  });
  it("preserves an operation's runtime error even when a newer healthy snapshot matches its draft", async () => {
    const h = setup(); const draft = fresh().settings; h.c.edit(draft);
    let resolve!: (s: ReminderSnapshot) => void; h.command.mockReturnValue(new Promise<ReminderSnapshot>(r => { resolve = r; })); const save = h.c.save();
    h.c.receive(fresh(9)); const failed = fresh(8); failed.runtimeError = "invalidTime"; resolve(failed); await save;
    expect(h.state().dirty).toBe(true); expect(h.state().notice).toContain("未能");
  });
  it("close drops unsubmitted edits, and reopen fences an in-flight submission", async () => {
    const h = setup(); const draft = fresh().settings; draft.snoozeMinutes = 30; h.c.edit(draft); await h.c.close(); expect(h.close).toHaveBeenCalledOnce(); expect(h.state().dirty).toBe(false);
    h.c.edit(draft); let resolve!: (s: ReminderSnapshot) => void; h.command.mockReturnValue(new Promise<ReminderSnapshot>(r => { resolve = r; })); const saving = h.c.save();
    h.c.reopen(fresh(10)); const newDraft = fresh().settings; newDraft.snoozeMinutes = 50; h.c.edit(newDraft); const old = fresh(2); old.settings = draft; resolve(old); await saving;
    expect(h.state().draft?.snoozeMinutes).toBe(50); expect(h.state().notice).not.toContain("已保存");
  });
  it("keeps an unsaved draft when native hide rejects", async () => {
    const h = setup(); const draft = fresh().settings; draft.snoozeMinutes = 37; h.c.edit(draft);
    h.close.mockRejectedValue(Error("hideFailed")); await h.c.close();
    expect(h.state().draft?.snoozeMinutes).toBe(37);
    expect(h.state().dirty).toBe(true);
    expect(h.state().notice).toContain("无法关闭");
  });
  it("rejects invalid intervals and equal daily times without submitting", async () => {
    const h = setup(); const draft = fresh().settings; draft.activeHours = { kind: "daily", start: 0, end: 0 }; h.c.edit(draft); await h.c.save(); expect(h.state().notice).toContain("全天");
    draft.activeHours = { kind: "allDay" }; draft.items[0].intervalMinutes = NaN; h.c.edit(draft); await h.c.save(); expect(h.command).not.toHaveBeenCalled();
  });
  it("ignores a failed save after disposal", async () => {
    const h = setup(); let reject!: (e: Error) => void; h.command.mockReturnValue(new Promise((_, r) => { reject = r; })); const save = h.c.save(); h.c.dispose(); const before = h.state(); reject(Error("late")); await save; expect(h.state()).toBe(before);
  });
});
