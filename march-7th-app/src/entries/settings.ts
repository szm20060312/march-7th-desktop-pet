import { createDomReminderSettings } from "../adapters/dom-reminder-settings";
import { connectReminders } from "../adapters/tauri-reminders";
import { hideSettingsWindow } from "../adapters/tauri-reminder-window";
import { createReminderSettingsController } from "../application/reminder-settings";
import { mountBuildInfo } from "../adapters/dom-build-info";
import { cancelLocalBackup, confirmLocalBackup, exportLocalBackup, selectLocalBackup } from "../adapters/tauri-local-backup";
import { createLocalBackupController, type BackupPreview } from "../application/local-backup";
import { createDomFocusControls } from "../adapters/dom-focus-controls";
import { connectFocus } from "../adapters/tauri-focus";
import { createFocusControls } from "../application/focus-controls";

const backupButton = document.getElementById("export-backup") as HTMLButtonElement;
const importButton = document.getElementById("import-backup") as HTMLButtonElement;
const cancelImportButton = document.getElementById("cancel-import") as HTMLButtonElement;
const confirmImportButton = document.getElementById("confirm-import") as HTMLButtonElement;
const previewRegion = document.getElementById("backup-preview") as HTMLElement;
const backupStatus = document.getElementById("backup-status") as HTMLElement;
const previewFields = Object.fromEntries(["character", "placement", "water", "move", "eyes", "hours", "pending", "reminder-state", "focus"].map(id => [id, document.getElementById(`import-${id}`) as HTMLElement])) as Record<string, HTMLElement>;
const minute = (value: number) => `${String(Math.floor(value / 60)).padStart(2, "0")}:${String(value % 60).padStart(2, "0")}`;
function renderPreview(preview: BackupPreview) {
  previewFields.character.textContent = ({ "march-7th": "三月七", "raiden-shogun": "雷电将军" } as Record<string, string>)[preview.selectedCharacterId] ?? "未知角色";
  previewFields.placement.textContent = preview.hasDesktopPlacement ? "包含已保存位置" : "未保存位置";
  for (const id of ["water", "move", "eyes"] as const) {
    const item = preview.reminders.items.find(row => row.id === id);
    previewFields[id].textContent = item ? `${item.enabled ? "开启" : "关闭"} · 每 ${item.intervalMinutes} 分钟` : "无效数据";
  }
  const hours = preview.reminders.activeHours;
  previewFields.hours.textContent = hours.kind === "allDay" ? "全天" : `${minute(hours.start)}—${minute(hours.end)}`;
  previewFields.pending.textContent = `待处理 ${preview.reminders.pendingCount} 项`;
  const quiet = preview.reminders.quietUntilUtcMs === null ? "无安静期" : `安静至 ${new Date(preview.reminders.quietUntilUtcMs).toLocaleString()}`;
  previewFields["reminder-state"].textContent = `${preview.reminders.paused ? "已暂停" : "运行中"} · ${quiet} · 稍后 ${preview.reminders.snoozeMinutes} 分钟${preview.reminders.snoozePending ? " · 有稍后待提示" : ""}`;
  previewFields.focus.textContent = preview.focus.defaultedFromV1 ? "旧版 v1 备份：将恢复为空专注会话" : ({ idle: "无专注会话", running: "进行中", paused: "已暂停", interrupted: "待确认中断", finished: "已结束" } as const)[preview.focus.status];
}
const backup = createLocalBackupController({
  exportNow: exportLocalBackup,
  select: selectLocalBackup, confirm: confirmLocalBackup, cancel: cancelLocalBackup,
  render(state) {
    backupButton.disabled = state.phase !== "idle";
    backupButton.textContent = state.phase === "exporting" ? "正在导出…" : "导出备份…";
    importButton.disabled = !["idle", "preview"].includes(state.phase);
    importButton.textContent = state.phase === "selecting" ? "正在选择…" : "导入备份…";
    previewRegion.hidden = state.selection === null;
    if (state.selection) renderPreview(state.selection.preview);
    confirmImportButton.disabled = state.phase !== "preview";
    cancelImportButton.disabled = state.phase !== "preview";
    backupStatus.textContent = state.message;
  },
});
const onBackupClick = () => { void backup.exportNow(); };
const onImportClick = () => { void backup.select(); };
const onConfirmImport = () => { void backup.confirm(); };
const onCancelImport = () => { void backup.cancel(); };
backupButton.addEventListener("click", onBackupClick);
importButton.addEventListener("click", onImportClick);
confirmImportButton.addEventListener("click", onConfirmImport);
cancelImportButton.addEventListener("click", onCancelImport);

const view = createDomReminderSettings(document);
const focusView = createDomFocusControls(document);
const focusControls = createFocusControls({ render: focusView.render, command: command => focusConnection.command(command), now: Date.now, monotonicNow: () => performance.now() });
const focusConnection = connectFocus({
  select: focusControls.receive,
  reportError(error) {
    console.error("Focus settings connection failed", error);
    if (error.stage !== "command") focusControls.error(error.stage === "listen" ? { code: "workerUnavailable" } : error.cause);
  },
});
const unbindFocus = focusView.bind(focusControls);
const focusTicker = setInterval(() => focusControls.tick(), 1000);
const controller = createReminderSettingsController({ render: view.render, command: command => connection.command(command), close: async () => { backup.close(); focusConnection.close(); focusControls.close(); await hideSettingsWindow(); } });
const connection = connectReminders({
  select: controller.receive,
  opened(intent) { controller.reopen(); backup.reopen(); focusControls.reopen(); void connection.refresh(); void focusConnection.reopen(); if (intent.target === "focus") document.getElementById("focus-section")?.scrollIntoView?.({ block: "start" }); },
  reportError(error) {
    console.error("Reminder settings connection failed", error);
    // Command feedback belongs to the controller's submitting session. The
    // connection may outlive that session when the native window is hidden.
    if (error.stage !== "command") controller.error(error.recovery === "restart" ? "提醒连接未建立，请重启应用。" : "读取或操作失败，请重试；也可从托盘重新打开设置。");
  },
});
const unbind = view.bind(controller);
const disposeBuildInfo = mountBuildInfo(document);
function dispose() { clearInterval(focusTicker); focusConnection.dispose(); focusControls.dispose(); unbindFocus(); connection.dispose(); controller.dispose(); backup.dispose(); backupButton.removeEventListener("click", onBackupClick); importButton.removeEventListener("click", onImportClick); confirmImportButton.removeEventListener("click", onConfirmImport); cancelImportButton.removeEventListener("click", onCancelImport); unbind(); disposeBuildInfo(); window.removeEventListener("pagehide", dispose); }
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
