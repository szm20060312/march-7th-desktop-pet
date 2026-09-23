import { expect, it, vi } from "vitest";
import { createDomReminderView } from "./dom-reminder-view";
import { documentDouble } from "../../test/dom-fixture";
it("retains actual button nodes and handlers in the same presentation, appending new rows", () => {
  const dom = documentDouble(); const view = createDomReminderView(dom.document); const complete = vi.fn().mockResolvedValue(undefined);
  const unbind = view.bind({ complete, snooze: vi.fn(), dismiss: vi.fn() });
  const base = { presentationId: 1, disabled: false, emptyText: "暂无待处理提醒", error: "" };
  view.render({ ...base, rows: [{ id: "water", done: false, busy: false }, { id: "move", done: false, busy: false }] });
  const first = dom.get("reminder-rows").children[0]; const move = dom.get("reminder-rows").children[1];
  view.render({ ...base, rows: [{ id: "water", done: true, busy: false }, { id: "move", done: false, busy: false }, { id: "eyes", done: false, busy: false }] });
  expect(dom.get("reminder-rows").children[0]).toBe(first); expect(dom.get("reminder-rows").children[1]).toBe(move); expect(first.children[1].disabled).toBe(true);
  move.children[1].dispatch("click"); expect(complete).toHaveBeenCalledWith("move");
  view.render({ ...base, presentationId: 2, rows: [{ id: "eyes", done: false, busy: false }] }); expect(dom.get("reminder-rows").children).toHaveLength(1);
  unbind(); move.children[1].dispatch("click"); expect(complete).toHaveBeenCalledOnce();
});
