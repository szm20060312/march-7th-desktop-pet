import { describe, expect, it, vi } from "vitest";
import { createReminderPresentation, type ReminderViewState } from "./reminder-presentation";
import { fresh } from "../../test/reminder-fixture";
import type { ReminderId, ReminderCommand } from "../domain/reminder";
function batch(revision: number, id: number, items: ReminderId[]) { const s = fresh(revision); s.presentation = { id, mode: "manual", items, closesAt: null }; s.progress.forEach(p => { p.pending = items.includes(p.id); }); return s; }
describe("reminder presentation", () => {
  it("keeps completed row positions, appends arrivals, rebuilds a new batch and never imports omitted pending items", async () => {
    let state!: ReminderViewState; const command = vi.fn().mockResolvedValue(undefined); const ready = vi.fn().mockResolvedValue(undefined);
    const c = createReminderPresentation({ render: s => { state = s; }, command, ready });
    c.receive(batch(1, 1, ["water", "move"])); await c.complete("water"); expect(state.rows[0].done).toBe(false);
    c.receive(batch(2, 1, ["move", "eyes"])); expect(state.rows.map(r => [r.id, r.done])).toEqual([["water", true], ["move", false], ["eyes", false]]);
    await c.complete("water"); expect(command).toHaveBeenCalledTimes(1);
    const next = batch(3, 2, ["eyes"]); next.progress[0].pending = true; c.receive(next); expect(state.rows.map(r => r.id)).toEqual(["eyes"]);
    expect(ready).toHaveBeenLastCalledWith(2);
  });
  it("completes each of three stable rows only via snapshots", async () => {
    let state!: ReminderViewState; let revision = 1; let remaining: ReminderId[] = ["water", "move", "eyes"];
    const command = vi.fn(async (cmd: ReminderCommand) => { if (cmd.type === "complete") remaining = remaining.filter(id => id !== cmd.id); return batch(++revision, 1, remaining); });
    const c = createReminderPresentation({ render: s => { state = s; }, command, ready: async () => {} }); c.receive(batch(1, 1, remaining));
    for (const id of ["water", "move", "eyes"] as const) await c.complete(id);
    expect(state.rows.map(r => r.id)).toEqual(["water", "move", "eyes"]); expect(state.rows.every(r => r.done)).toBe(true);
  });
  it("captures dismiss ID before async work and ignores old replies", async () => {
    let state!: ReminderViewState; let resolve!: (s: ReturnType<typeof batch>) => void; const command = vi.fn(() => new Promise<ReturnType<typeof batch>>(r => { resolve = r; }));
    const c = createReminderPresentation({ render: s => { state = s; }, command, ready: async () => {} }); c.receive(batch(1, 7, ["water"])); const dismiss = c.dismiss(); c.receive(batch(3, 8, ["eyes"])); resolve(batch(2, 7, [])); await dismiss;
    expect(command).toHaveBeenCalledWith({ type: "dismiss", presentationId: 7 }); expect(state.presentationId).toBe(8);
  });
  it("acknowledges after render, handles empty/null, and suppresses late ready errors", async () => {
    const order: string[] = []; let state!: ReminderViewState; let reject!: (e: Error) => void;
    const c = createReminderPresentation({ render: s => { state = s; order.push("render"); }, command: vi.fn(), ready: () => { order.push("ready"); return new Promise((_, r) => { reject = r; }); } });
    c.receive(batch(1, 1, [])); expect(order.slice(-2)).toEqual(["render", "ready"]); expect(state.emptyText).toBe("暂无待处理提醒"); c.receive(fresh(2)); expect(state.presentationId).toBeNull(); expect(state.rows).toEqual([]); c.dispose(); reject(Error("late")); await Promise.resolve(); expect(state.error).toBe("");
  });
});

it("presents a focus rest message and authoritative pending count without completing reminders", async () => {
  let state!: ReminderViewState; const command=vi.fn().mockResolvedValue(undefined);
  const c=createReminderPresentation({render:s=>{state=s;},command,ready:async()=>{}});
  const snapshot=batch(7,4,[]); snapshot.presentation={...snapshot.presentation!,focusCompleted:true}; snapshot.progress[0].pending=true; snapshot.progress[1].pending=true;
  c.receive(snapshot); expect(state.focusCompleted).toBe(true); expect(state.pendingCount).toBe(2); expect(state.rows).toEqual([]);
  await c.snooze(); expect(command).toHaveBeenCalledWith({type:"showPending"});
  expect(snapshot.progress.filter(p=>p.pending)).toHaveLength(2);
});
