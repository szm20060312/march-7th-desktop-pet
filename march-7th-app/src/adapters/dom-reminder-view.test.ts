import { expect, it, vi } from "vitest";
import { createDomReminderView } from "./dom-reminder-view";
import { documentDouble } from "../../test/dom-fixture";
import { characterCatalog } from "../characters/catalog";
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

it("focus completion has one pending entry action and restores daily reminder labels", () => {
  const dom=documentDouble(); const view=createDomReminderView(dom.document);const snooze=vi.fn();view.bind({snooze,dismiss:vi.fn(),complete:vi.fn()});
  const base={presentationId:7,rows:[],disabled:false,emptyText:"暂无待处理提醒",error:""};
  view.render({...base,focusCompleted:true,pendingCount:3});expect(dom.get("reminder-title").textContent).toContain("休息一下");expect(dom.get("reminder-empty").textContent).toContain("3 项");expect(dom.get("snooze-reminder").textContent).toBe("查看待处理");dom.get("snooze-reminder").dispatch("click");expect(snooze).toHaveBeenCalledOnce();
  view.render({...base,presentationId:8});expect(dom.get("snooze-reminder").textContent).toBe("稍后提醒");
});

it("presents an automatic reminder as the selected character's speech without losing actions", () => {
  const dom = documentDouble(); const view = createDomReminderView(dom.document);
  const complete = vi.fn(); const snooze = vi.fn(); view.bind({ complete, snooze, dismiss: vi.fn() });
  const base = { presentationId: 7, mode: "automatic" as const, rows: [{ id: "water" as const, done: false, busy: false }], disabled: false, emptyText: "", error: "" };
  view.setCharacter(characterCatalog.characters[0]); view.render(base);
  expect(dom.get("reminder-speaker").textContent).toBe("三月七");
  expect(characterCatalog.characters[0].reminderPrompts.water).toContain(dom.get("reminder-prompt").textContent);
  dom.get("reminder-rows").children[0].children[1].dispatch("click"); expect(complete).toHaveBeenCalledWith("water");
  dom.get("snooze-reminder").dispatch("click"); expect(snooze).toHaveBeenCalledOnce();
  view.setCharacter(characterCatalog.characters[1]); view.render(base);
  expect(dom.get("reminder-speaker").textContent).toBe("雷电将军");
  expect(characterCatalog.characters[1].reminderPrompts.water).toContain(dom.get("reminder-prompt").textContent);
  view.render({ ...base, mode: "manual" });
  expect(dom.get("reminder-prompt").textContent).toBe("待处理提醒");
});
