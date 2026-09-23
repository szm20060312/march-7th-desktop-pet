export type BackupResult = "saved" | "cancelled";

const messages: Record<string, string> = {
  backupUnavailable: "当前状态无法完整导出，请检查角色、提醒和位置保存状态后重试。",
  backupAlreadyExists: "同名文件已存在，请另选一个文件名。",
  backupWriteFailed: "写入未完成；若看到同名文件，请手动删除后重试。",
  backupBusy: "已有一次导出正在进行，请稍候。",
  backupStale: "窗口已关闭，本次导出已取消。",
};

export function createLocalBackupController(ports: {
  exportNow(): Promise<BackupResult>;
  render(state: { busy: boolean; message: string }): void;
}) {
  let session = 0; let busy = false; let message = ""; let disposed = false;
  const render = () => { if (!disposed) ports.render({ busy, message }); };
  const reset = () => { session++; busy = false; message = ""; render(); };
  render();
  return {
    async exportNow() {
      if (disposed || busy) return;
      const token = session; busy = true; message = "正在选择保存位置…"; render();
      try {
        const result = await ports.exportNow();
        if (disposed || token !== session) return;
        message = result === "saved" ? "保存成功。此文件只在你选择的位置，不会自动上传。" : "已取消，没有写入文件。";
      } catch (error) {
        if (disposed || token !== session) return;
        const code = typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
        message = messages[code] ?? "导出失败，请重试。";
      } finally {
        if (!disposed && token === session) { busy = false; render(); }
      }
    },
    reopen: reset,
    close: reset,
    dispose() { disposed = true; session++; },
  };
}
