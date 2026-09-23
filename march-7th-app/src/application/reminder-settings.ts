import { copySettings, sameSettings, type ReminderCommand, type ReminderSettings, type ReminderSnapshot } from "../domain/reminder";
export interface SettingsViewState {
  draft: ReminderSettings | null;
  snapshot: ReminderSnapshot | null;
  dirty: boolean;
  saving: boolean;
  editable: boolean;
  pauseBusy: boolean;
  notice: string;
}
export function settingsValidation(settings: ReminderSettings): string {
  if (settings.items.some(i => !Number.isInteger(i.intervalMinutes) || i.intervalMinutes < 1 || i.intervalMinutes > 1440)) return "提醒间隔请填写 1–1440 的整数分钟。";
  if (!Number.isInteger(settings.snoozeMinutes) || settings.snoozeMinutes < 1 || settings.snoozeMinutes > 120) return "稍后提醒请填写 1–120 的整数分钟。";
  const hours = settings.activeHours;
  if (hours.kind === "daily") {
    if (hours.start === hours.end) return "起止时间不能相同；如需整天提醒，请选择全天。";
    if (![hours.start, hours.end].every(v => Number.isInteger(v) && v >= 0 && v < 1440)) return "请填写完整的每日起止时间。";
  }
  return "";
}
export function createReminderSettingsController(ports: {
  render(state: SettingsViewState): void;
  command(command: ReminderCommand): Promise<ReminderSnapshot | undefined>;
  close(): Promise<void> | void;
}) {
  let snapshot: ReminderSnapshot | null = null; let draft: ReminderSettings | null = null;
  let dirty = false; let saving = false; let pauseBusy = false; let notice = ""; let disposed = false; let session = 0;
  const editable = () => !!snapshot && !snapshot.stopped && !["loading", "readOnly"].includes(snapshot.persistence.status);
  const render = () => { if (!disposed) ports.render({ snapshot, draft: draft && copySettings(draft), dirty, saving, pauseBusy, notice, editable: editable() && !saving }); };
  const receive = (next: ReminderSnapshot) => {
    if (disposed || (snapshot && next.revision < snapshot.revision)) return;
    if (!saving && snapshot && next.revision > snapshot.revision) notice = "";
    snapshot = next;
    if (!dirty && !saving) draft = copySettings(next.settings);
    render();
  };
  const reset = () => { session++; saving = false; pauseBusy = false; dirty = false; notice = ""; draft = snapshot && copySettings(snapshot.settings); render(); };
  render();
  return {
    receive,
    edit(value: ReminderSettings) { if (disposed || !editable() || saving) return; draft = copySettings(value); dirty = true; notice = ""; render(); },
    async save() {
      if (disposed || !editable() || saving || !draft) return;
      notice = settingsValidation(draft); if (notice) { render(); return; }
      const submitted = copySettings(draft); const token = session; saving = true; notice = ""; render();
      try {
        const reply = await ports.command({ type: "updateSettings", settings: submitted });
        if (disposed || token !== session) return;
        if (reply) receive(reply);
        const actual = snapshot;
        if (!reply || reply.runtimeError || reply.stopped || !sameSettings(reply.settings, submitted) || !actual || actual.stopped || actual.runtimeError || !sameSettings(actual.settings, submitted) || ["readOnly", "loading", "default"].includes(actual.persistence.status) || ["readOnly", "loading", "default"].includes(reply.persistence.status)) {
          dirty = true; notice = "未能确认本次设置已应用，请检查状态后重试。";
        } else if (actual.persistence.status === "unsaved" || reply.persistence.status === "unsaved") {
          dirty = true; notice = "本次设置已应用，但未保存到本机；请重试保存。";
        } else {
          dirty = false; draft = copySettings(actual.settings); notice = "设置已保存。";
        }
      } catch { if (!disposed && token === session) { dirty = true; notice = "保存失败，请重试；当前草稿仍保留。"; } }
      finally { if (!disposed && token === session) { saving = false; render(); } }
    },
    async togglePaused() {
      if (disposed || !editable() || pauseBusy || !snapshot) return;
      const token = session; pauseBusy = true; render();
      try { const reply = await ports.command({ type: "setPaused", paused: !snapshot.paused }); if (!disposed && token === session && reply) receive(reply); }
      catch { if (!disposed && token === session) notice = "暂停状态未能更新，请重试。"; }
      finally { if (!disposed && token === session) { pauseBusy = false; render(); } }
    },
    reopen(latest?: ReminderSnapshot) { if (disposed) return; if (latest) receive(latest); reset(); },
    async close() { if (disposed) return; reset(); try { await ports.close(); } catch { if (!disposed) { notice = "无法关闭窗口，请重试。"; render(); } } },
    error(message: string) { if (!disposed) { notice = message; render(); } },
    dispose() { disposed = true; session++; },
  };
}
