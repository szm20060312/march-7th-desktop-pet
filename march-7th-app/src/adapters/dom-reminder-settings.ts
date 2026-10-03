import type { SettingsViewState } from "../application/reminder-settings";
import { reminderIds, type ReminderSettings } from "../domain/reminder";
export function createDomReminderSettings(root: Document) {
  const element = <T extends HTMLElement>(id: string) => { const el = root.getElementById(id); if (!el) throw new Error(`Missing settings element ${id}`); return el as T; };
  const input = (id: string) => element<HTMLInputElement>(id);
  const form = element<HTMLFormElement>("settings-form"); const kind = element<HTMLSelectElement>("hours-kind");
  const start = input("hours-start"); const end = input("hours-end"); const snooze = input("snooze-minutes");
  const fields = reminderIds.map(id => ({ id, enabled: input(`${id}-enabled`), interval: input(`${id}-interval`) }));
  const pause = element<HTMLButtonElement>("pause"); const save = element<HTMLButtonElement>("save-settings"); const close = element<HTMLButtonElement>("close-settings");
  const time = (minutes: number) => `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
  const minutes = (value: string) => { if (!/^\d{2}:\d{2}$/.test(value)) return NaN; const [h, m] = value.split(":").map(Number); return h * 60 + m; };
  const setValue = (el: HTMLInputElement, value: string) => { if (el.value !== value) el.value = value; };
  const numeric = (value: number) => Number.isFinite(value) ? String(value) : "";
  const read = (): ReminderSettings => ({ items: fields.map(f => ({ id: f.id, enabled: f.enabled.checked, intervalMinutes: f.interval.valueAsNumber })), activeHours: kind.value === "allDay" ? { kind: "allDay" } : { kind: "daily", start: minutes(start.value), end: minutes(end.value) }, snoozeMinutes: snooze.valueAsNumber });
  return {
    render(state: SettingsViewState) {
      const { draft, snapshot } = state;
      if (draft) {
        for (const f of fields) { const item = draft.items.find(v => v.id === f.id)!; f.enabled.checked = item.enabled; setValue(f.interval, numeric(item.intervalMinutes)); }
        kind.value = draft.activeHours.kind;
        if (draft.activeHours.kind === "daily") { setValue(start, Number.isFinite(draft.activeHours.start) ? time(draft.activeHours.start) : ""); setValue(end, Number.isFinite(draft.activeHours.end) ? time(draft.activeHours.end) : ""); }
        setValue(snooze, numeric(draft.snoozeMinutes));
      }
      element<HTMLFieldSetElement>("configuration").disabled = !state.editable;
      element("daily-hours").hidden = draft?.activeHours.kind !== "daily";
      element("hours-hint").hidden = draft?.activeHours.kind !== "daily";
      pause.disabled = !snapshot || snapshot.stopped || ["loading", "readOnly"].includes(snapshot.persistence.status) || state.pauseBusy;
      pause.textContent = snapshot?.paused ? "恢复提醒" : "暂停提醒";
      element("pause-state").textContent = !snapshot ? "正在读取状态" : snapshot.stopped ? "提醒服务已停止" : snapshot.paused ? "提醒已暂停" : "提醒未暂停";
      let status = "正在读取设置…";
      if (snapshot) {
        status = snapshot.stopped ? "提醒服务已停止，请重启应用。" : snapshot.persistence.status === "loading" ? "正在读取设置，请稍候。" : snapshot.persistence.status === "readOnly" ? "设置文件受保护，当前只读；请检查本机配置后重启。" : snapshot.runtimeError ? "提醒运行出错，本次操作未确认；请检查系统时间后重试。" : snapshot.persistence.status === "unsaved" ? "当前设置已应用，但未保存到本机；请重试保存。" : state.dirty ? "有未提交的草稿；关闭将丢弃草稿。" : snapshot.persistence.status === "default" ? "当前为默认设置，所有提醒默认关闭。" : "当前设置已保存到本机。";
      }
      element("settings-status").textContent = status;
      element("settings-notice").textContent = state.notice;
      save.disabled = !state.editable; save.textContent = state.saving ? "正在保存…" : "保存设置";
    },
    bind(actions: { edit(settings: ReminderSettings): void; save(): Promise<void>; togglePaused(): Promise<void>; close(): Promise<void> }) {
      const edit = () => actions.edit(read()); const submit = (event: Event) => { event.preventDefault(); void actions.save(); }; const toggle = () => { void actions.togglePaused(); }; const hide = () => { void actions.close(); };
      form.addEventListener("input", edit); form.addEventListener("change", edit); form.addEventListener("submit", submit); pause.addEventListener("click", toggle); close.addEventListener("click", hide);
      return () => { form.removeEventListener("input", edit); form.removeEventListener("change", edit); form.removeEventListener("submit", submit); pause.removeEventListener("click", toggle); close.removeEventListener("click", hide); };
    },
  };
}
