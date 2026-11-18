import {
  clearDebugLogs,
  getDebugLogs,
  getDebugState,
  listenDebugLogs,
  triggerDebugPetAction,
  type DebugLogEntry,
  type DebugPetAction,
} from "./tauri-debug";

const UNLOCK_CLICKS = 8;
const TAP_WINDOW_MS = 1500;

export function mountDevConsole(document: Document, now: () => number = () => Date.now()): () => void {
  const versionEl = document.getElementById("build-version");
  const statusEl = document.getElementById("build-status");
  const panel = document.getElementById("dev-console");
  const logOutput = document.getElementById("dev-log-output");
  const stateOutput = document.getElementById("dev-state-output");
  if (!versionEl || !panel || !logOutput || !stateOutput) return () => {};

  let disposed = false;
  let unlocked = false;
  let clicks = 0;
  let lastClickAt = 0;
  let stopStream: (() => void) | undefined;
  const entries: DebugLogEntry[] = [];

  const renderLogs = () => {
    logOutput.textContent = entries.length
      ? entries.map(e => `[${e.timestamp}] [${e.category}] ${e.message}`).join("\n")
      : "暂无日志。";
    logOutput.scrollTop = logOutput.scrollHeight;
  };

  const refreshAll = async () => {
    try {
      const [logs, state] = await Promise.all([getDebugLogs(), getDebugState()]);
      if (disposed) return;
      entries.splice(0, entries.length, ...logs);
      renderLogs();
      stateOutput.textContent = JSON.stringify(state, null, 2);
    } catch (error) {
      if (!disposed) stateOutput.textContent = `读取调试状态失败: ${String(error)}`;
    }
  };

  const unlock = () => {
    if (unlocked) return;
    unlocked = true;
    panel.hidden = false;
    if (statusEl) statusEl.textContent = "开发者控制台已解锁。";
    stopStream = listenDebugLogs(entry => {
      entries.push(entry);
      if (entries.length > 200) entries.shift();
      renderLogs();
    });
    void refreshAll();
  };

  const onVersionClick = () => {
    if (disposed || unlocked) return;
    const t = now();
    clicks = t - lastClickAt <= TAP_WINDOW_MS ? clicks + 1 : 1;
    lastClickAt = t;
    if (clicks >= UNLOCK_CLICKS) {
      unlock();
    } else if (clicks >= 5 && statusEl) {
      statusEl.textContent = `再点击 ${UNLOCK_CLICKS - clicks} 次打开开发者控制台`;
    }
  };

  const onActionClick = (event: Event) => {
    const target = event.target as HTMLElement | null;
    const button = target?.closest<HTMLButtonElement>("button[data-debug-action]");
    if (!button || disposed) return;
    const action = button.dataset.debugAction as DebugPetAction;
    void triggerDebugPetAction(action).then(() => refreshAll());
  };

  const refreshBtn = document.getElementById("dev-refresh");
  const clearBtn = document.getElementById("dev-clear-logs");
  const onRefresh = () => { void refreshAll(); };
  const onClear = () => {
    void clearDebugLogs().then(() => {
      if (!disposed) {
        entries.length = 0;
        renderLogs();
      }
    });
  };

  versionEl.addEventListener("click", onVersionClick);
  panel.addEventListener("click", onActionClick);
  refreshBtn?.addEventListener("click", onRefresh);
  clearBtn?.addEventListener("click", onClear);

  return () => {
    disposed = true;
    stopStream?.();
    versionEl.removeEventListener("click", onVersionClick);
    panel.removeEventListener("click", onActionClick);
    refreshBtn?.removeEventListener("click", onRefresh);
    clearBtn?.removeEventListener("click", onClear);
  };
}
