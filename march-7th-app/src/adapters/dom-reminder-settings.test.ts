import { expect, it, vi } from "vitest";
import { createDomReminderSettings } from "./dom-reminder-settings";
import { documentDouble } from "../../test/dom-fixture";
import { fresh } from "../../test/reminder-fixture";
it("uses labeled ID fields, strips daily fields for allDay, and submits only through explicit save", () => {
  const dom = documentDouble(); const view = createDomReminderSettings(dom.document); const snapshot = fresh();
  view.render({ snapshot, draft: snapshot.settings, dirty: false, saving: false, editable: true, pauseBusy: false, quietBusy: false, notice: "" });
  const actions = { edit: vi.fn(), save: vi.fn(), togglePaused: vi.fn(), clearQuiet: vi.fn(), close: vi.fn() }; const stop = view.bind(actions);
  expect(dom.get("clear-quiet").disabled).toBe(true);
  expect(dom.get("quiet-state").textContent).toContain("未处于安静期");
  snapshot.quiet = { until: Date.UTC(2026, 0, 1, 12, 30), durationMinutes: 10 };
  view.render({ snapshot, draft: snapshot.settings, dirty: false, saving: false, editable: true, pauseBusy: false, quietBusy: false, notice: "" });
  expect(dom.get("clear-quiet").disabled).toBe(false);
  expect(dom.get("quiet-state").textContent).toContain("稍后安静中");
  dom.get("clear-quiet").dispatch("click"); expect(actions.clearQuiet).toHaveBeenCalledOnce();
  dom.get("hours-kind").value = "allDay"; dom.get("water-enabled").checked = true; dom.get("settings-form").dispatch("change");
  const draft = actions.edit.mock.calls[0][0]; expect(draft.activeHours).toEqual({ kind: "allDay" }); expect(draft.items[0].enabled).toBe(true); expect(actions.save).not.toHaveBeenCalled();
  dom.get("settings-form").dispatch("submit"); expect(actions.save).toHaveBeenCalledOnce(); stop(); dom.get("settings-form").dispatch("submit"); expect(actions.save).toHaveBeenCalledOnce();
});
it("exposes loading/readOnly/unsaved/runtime errors and does not enable blocked settings", () => {
  const dom = documentDouble(); const view = createDomReminderSettings(dom.document); const snapshot = fresh();
  for (const status of ["loading", "readOnly", "unsaved"] as const) {
    snapshot.persistence.status = status;
    view.render({ snapshot, draft: snapshot.settings, dirty: false, saving: false, editable: status === "unsaved", pauseBusy: false, quietBusy: false, notice: "" });
    expect(dom.get("configuration").disabled).toBe(status !== "unsaved"); expect(dom.get("settings-status").textContent).toContain({ loading: "读取", readOnly: "只读", unsaved: "未保存" }[status]);
  }
  snapshot.persistence.status = "saved"; snapshot.runtimeError = "invalidTime"; view.render({ snapshot, draft: snapshot.settings, dirty: true, saving: false, editable: true, pauseBusy: false, quietBusy: false, notice: "" }); expect(dom.get("settings-status").textContent).toContain("未确认");
});
