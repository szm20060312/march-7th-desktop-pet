import type { FocusChange, FocusCommand, FocusSnapshot } from "../domain/focus";

export type FocusAction = FocusCommand["type"];
export type FocusViewState = { snapshot: FocusSnapshot | null; durationMinutes: number; displayRemainingMs: number | null; busy: boolean; notice: string };
const errorCode = (error: unknown) => typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
const errorMessage = (error: unknown) => {
  switch (errorCode(error)) {
    case "readOnly": case "invalidFile": case "unsupportedVersion": case "fileChanged": return "专注记录处于保护模式，当前无法更改；请检查本机数据后重启。";
    case "writeFailed": case "directoryUnavailable": return "专注记录未写入，操作未确认；请检查本机存储后重试。";
    case "stopped": case "workerUnavailable": return "专注服务暂不可用，请重启应用。";
    case "queueFull": return "操作较多，请稍候再试；状态未确认。";
    case "invalidDuration": return "时长须为 1–240 分钟。";
    case "alreadyActive": case "invalidTransition": return "专注状态已变化，请按当前状态重新操作。";
    default: return "操作未确认，请重试；当前状态以专注服务为准。";
  }
};

export function createFocusControls(ports: { command(command: FocusCommand): Promise<FocusSnapshot>; render(state: FocusViewState): void; now(): number }) {
  const now = ports.now;
  let snapshot: FocusSnapshot | null = null;
  let durationMinutes = 25;
  let receivedAt = 0;
  let notice = "";
  let diagnostic = false;
  let busy = false;
  let generation = 0;
  let disposed = false;
  let visible = true;
  const displayRemainingMs = () => {
    const session = snapshot?.data?.session;
    if (!session || !(session.status === "running" || session.status === "paused" || session.status === "interrupted")) return null;
    return session.status === "running" ? Math.max(0, session.remaining_ms - Math.max(0, now() - receivedAt)) : session.remaining_ms;
  };
  const render = () => { if (!disposed && visible) ports.render({ snapshot, durationMinutes, displayRemainingMs: displayRemainingMs(), busy, notice }); };
  render();
  return {
    receive(change: FocusChange) {
      if (disposed || snapshot && change.snapshot.revision < snapshot.revision) return;
      const stateChanged = snapshot?.revision !== change.snapshot.revision || snapshot?.data?.session.status !== change.snapshot.data?.session.status;
      if (stateChanged) receivedAt = now();
      snapshot = change.snapshot;
      if (change.error) { notice = errorMessage({ code: change.error }); diagnostic = true; }
      else if (diagnostic || stateChanged) { notice = ""; diagnostic = false; }
      render();
    },
    setDuration(value: number) { if (disposed || busy) return; durationMinutes = value; notice = ""; diagnostic = false; render(); },
    async act(type: FocusAction) {
      if (disposed || !visible || busy || !snapshot || snapshot.stopped || snapshot.error || !snapshot.data) return;
      const session = snapshot.data.session;
      const allowed = type === "start" ? ["idle", "finished"].includes(session.status)
        : type === "pause" ? session.status === "running"
        : type === "resume" ? ["paused", "interrupted"].includes(session.status)
        : type === "endEarly" || type === "abandon" ? ["running", "paused", "interrupted"].includes(session.status)
        : session.status === "finished" && session.outcome === "natural" && session.feedback === "pending";
      if (!allowed) return;
      if (type === "start" && (!Number.isSafeInteger(durationMinutes) || durationMinutes < 1 || durationMinutes > 240)) { notice = "时长须为 1–240 分钟。"; render(); return; }
      const token = generation;
      const command: FocusCommand = type === "start" ? { type, durationMs: durationMinutes * 60_000 } : { type };
      busy = true; notice = ""; diagnostic = false; render();
      try {
        const result = await ports.command(command);
        if (disposed || token !== generation) return;
        if (!snapshot || result.revision >= snapshot.revision) { snapshot = result; receivedAt = now(); }
      } catch (error) { if (!disposed && token === generation) { notice = errorMessage(error); diagnostic = false; } }
      finally { if (!disposed && token === generation) { busy = false; render(); } }
    },
    tick: render,
    error(error: unknown) { if (!disposed && visible) { notice = errorMessage(error); diagnostic = false; render(); } },
    close() { generation++; busy = false; visible = false; notice = ""; },
    reopen() { generation++; busy = false; visible = true; notice = ""; render(); },
    dispose() { disposed = true; generation++; },
  };
}
