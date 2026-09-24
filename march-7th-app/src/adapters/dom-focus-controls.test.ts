import { expect, it, vi } from "vitest";
import { createDomFocusControls } from "./dom-focus-controls";
import { documentDouble } from "../../test/dom-fixture";
import type { FocusViewState } from "../application/focus-controls";
import type { FocusSession } from "../domain/focus";

const state = (status: "idle" | "running" | "interrupted" | "finished", extra: object = {}): FocusViewState => ({
  snapshot: { revision: 2, data: { version: 2, task: null, session: { status, ...extra } as FocusSession }, error: null, stopped: false },
  durationMinutes: 25, taskName: "", displayRemainingMs: status === "running" ? 60_000 : null, busy: false, notice: "",
});

it("presents one main action, accessible secondary actions and clear session states", () => {
  const dom = documentDouble(); const view = createDomFocusControls(dom.document);
  view.render(state("idle"));
  expect(dom.get("focus-start").hidden).toBe(false);
  expect(dom.get("focus-duration").disabled).toBe(false);
  view.render(state("running", { duration_ms: 1_500_000, remaining_ms: 60_000, anchor_utc_ms: 1 }));
  expect(dom.get("focus-time").textContent).toBe("01:00");
  expect(dom.get("focus-pause").hidden).toBe(false);
  expect(dom.get("focus-duration").disabled).toBe(true);
  view.render(state("interrupted", { duration_ms: 1_500_000, remaining_ms: 50_000 }));
  expect(dom.get("focus-state").textContent).toContain("待确认");
  expect(dom.get("focus-resume").hidden).toBe(false);
  view.render(state("finished", { duration_ms: 1_500_000, outcome: "natural", feedback: "pending" }));
  expect(dom.get("focus-state").textContent).toContain("自然完成");
  expect(dom.get("focus-dismiss").hidden).toBe(false);
});

it("binds all explicit controls and detaches them on disposal", () => {
  const dom = documentDouble(); const view = createDomFocusControls(dom.document);
  const actions = { setDuration: vi.fn(), setTaskName: vi.fn(), act: vi.fn() }; const unbind = view.bind(actions);
  dom.get("focus-duration").value = "45"; dom.get("focus-duration").dispatch("input");
  expect(actions.setDuration).toHaveBeenCalledWith(45);
  for (const type of ["start", "pause", "resume", "endEarly", "abandon", "dismissFeedback"]) dom.get(`focus-${type === "endEarly" ? "end" : type === "dismissFeedback" ? "dismiss" : type}`).dispatch("click");
  expect(actions.act.mock.calls.map(([type]) => type)).toEqual(["start", "pause", "resume", "endEarly", "abandon", "dismissFeedback"]);
  unbind(); dom.get("focus-start").dispatch("click"); expect(actions.act).toHaveBeenCalledTimes(6);
});

it("keeps the authoritative pending count visible independently of focus controls", () => {
  const dom=documentDouble();const view=createDomFocusControls(dom.document);view.pending(3);view.render(state("finished",{outcome:"endedEarly",feedback:"none"}));
  expect(dom.get("focus-pending").textContent).toContain("3 项");view.pending(0);expect(dom.get("focus-pending").textContent).toContain("0 项");
});
it("task input writes plain text, mutually exclusive controls disappear, and the timer stays actionable", () => {
  const dom=documentDouble();const view=createDomFocusControls(dom.document);
  const running=state("running",{duration_ms:60000,remaining_ms:60000,anchor_utc_ms:0});
  running.snapshot!.data!.task={name:"<img src=x> 私人事项",status:"active"};view.render(running);
  expect(dom.get("focus-task-state").textContent).toBe("<img src=x> 私人事项 · 进行中");
  expect(dom.get("focus-task-complete").hidden).toBe(false);expect(dom.get("focus-task-abandon").hidden).toBe(false);
  expect(dom.get("focus-task-entry").hidden).toBe(true);
  running.snapshot!.data!.task.status="completed";view.render(running);
  expect(dom.get("focus-task-complete").hidden).toBe(true);expect(dom.get("focus-task-abandon").hidden).toBe(true);
  expect(dom.get("focus-pause").disabled).toBe(false);
});
