export type BackupResult = "saved" | "cancelled";
export type BackupPreview = {
  createdAtUtcMs: number;
  selectedCharacterId: string;
  hasDesktopPlacement: boolean;
  reminders: {
    items: { id: "water" | "move" | "eyes"; enabled: boolean; intervalMinutes: number }[];
    activeHours: { kind: "allDay" } | { kind: "daily"; start: number; end: number };
    snoozeMinutes: number;
    pendingCount: number;
    paused: boolean;
    quietUntilUtcMs: number | null;
    snoozePending: boolean;
  };
};
export type SelectedBackup = { ticket: number; preview: BackupPreview };
export type BackupPhase = "idle" | "exporting" | "selecting" | "preview" | "cancelling" | "confirming" | "pending";

const messages: Record<string, string> = {
  backupUnavailable: "当前状态不可用，请稍后重试。",
  backupAlreadyExists: "同名文件已存在，请另选一个文件名。",
  backupWriteFailed: "写入未完成；若看到同名文件，请手动删除后重试。",
  backupReadFailed: "无法读取所选文件，请重新选择本机备份。",
  backupTooLarge: "备份超过 2 MiB，请检查文件。",
  backupInvalid: "文件不是有效的本地备份。",
  backupInvalidTime: "备份时间无效，请检查文件。",
  backupUnsupportedVersion: "备份版本比当前应用更新，暂不能导入。",
  backupChecksumMismatch: "备份校验失败，文件可能已损坏。",
  dataSetInvalid: "备份中的角色、位置或提醒数据无效，未安排导入。",
  dataSetsUnavailable: "无法准备本地数据，原数据未改变，请稍后重试。",
  dataSetCreateFailed: "无法准备本地数据，原数据未改变，请稍后重试。",
  dataSetWriteFailed: "无法准备本地数据，原数据未改变，请稍后重试。",
  pendingImportWriteFailed: "无法安排下次启动导入，原数据未改变，请稍后重试。",
  directoryUnavailable: "本地数据目录不可用，无法安排导入。",
  pendingImportExists: "已有待导入备份，请先正常退出并重新打开应用。",
  backupBusy: "另一项本地数据操作正在进行，请稍候。",
  backupStale: "设置窗口已变化，本次操作已取消。",
};
const errorCode = (error: unknown) => typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";

export function createLocalBackupController(ports: {
  exportNow(): Promise<BackupResult>;
  select(): Promise<SelectedBackup | null>;
  confirm(ticket: number): Promise<{ restartRequired: boolean }>;
  cancel(ticket: number): Promise<void>;
  render(state: { phase: BackupPhase; message: string; selection: SelectedBackup | null }): void;
}) {
  let generation = 0;
  let phase: BackupPhase = "idle";
  let message = "";
  let selection: SelectedBackup | null = null;
  let disposed = false;
  const render = () => { if (!disposed) ports.render({ phase, message, selection }); };
  const reset = () => { generation++; phase = "idle"; message = ""; selection = null; render(); };
  render();
  return {
    async exportNow() {
      if (disposed || phase !== "idle") return;
      const token = generation; phase = "exporting"; message = "正在选择保存位置…"; render();
      try {
        const result = await ports.exportNow();
        if (disposed || token !== generation) return;
        message = result === "saved" ? "保存成功。此文件只在你选择的位置，不会自动上传。" : "已取消，没有写入文件。";
      } catch (error) {
        if (disposed || token !== generation) return;
        message = messages[errorCode(error)] ?? "导出失败，请重试。";
      } finally {
        if (!disposed && token === generation) { phase = "idle"; render(); }
      }
    },
    async select() {
      if (disposed || !["idle", "preview"].includes(phase)) return;
      const token = ++generation; selection = null; phase = "selecting"; message = "正在选择并检查备份…"; render();
      try {
        const chosen = await ports.select();
        if (disposed || token !== generation) return;
        if (chosen) { selection = chosen; phase = "preview"; message = "请核对覆盖范围；确认前不会更改本地数据。"; }
        else { phase = "idle"; message = "已取消，没有安排导入。"; }
      } catch (error) {
        if (disposed || token !== generation) return;
        phase = "idle"; message = messages[errorCode(error)] ?? "无法预览备份，请重新选择。";
      }
      render();
    },
    async confirm() {
      if (disposed || phase !== "preview" || !selection) return;
      const token = generation; const ticket = selection.ticket;
      phase = "confirming"; message = "正在安排下次启动导入…"; render();
      try {
        const result = await ports.confirm(ticket);
        if (disposed || token !== generation) return;
        if (!result.restartRequired) throw { code: "backupInvalidResponse" };
        selection = null; phase = "pending";
        message = "已安排导入，下次启动生效。当前会话保持不变；正常退出并重新打开后整体生效。原数据会保留。";
      } catch (error) {
        if (disposed || token !== generation) return;
        if (errorCode(error) === "backupStale") { selection = null; phase = "idle"; }
        else phase = "preview";
        message = messages[errorCode(error)] ?? "未能安排导入，原数据未改变；可重试。";
      }
      render();
    },
    async cancel() {
      if (disposed || phase !== "preview" || !selection) return;
      const token = generation; const ticket = selection.ticket;
      phase = "cancelling"; render();
      try { await ports.cancel(ticket); }
      catch (error) {
        if (disposed || token !== generation) return;
        if (errorCode(error) !== "backupStale") message = messages[errorCode(error)] ?? "取消未完成，请关闭设置后重试。";
      }
      if (disposed || token !== generation) return;
      selection = null; phase = "idle";
      if (!message || message.startsWith("请核对")) message = "已取消，没有安排导入。";
      render();
    },
    reopen: reset,
    close: reset,
    dispose() { disposed = true; generation++; },
  };
}
