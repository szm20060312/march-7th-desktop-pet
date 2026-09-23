import type { FocusAction, FocusViewState } from "../application/focus-controls";
export function createDomFocusControls(root: Document) {
  const get = <T extends HTMLElement>(id: string) => { const element = root.getElementById(id); if (!element) throw Error(`Missing focus element ${id}`); return element as T; };
  const duration = get<HTMLInputElement>("focus-duration");
  const status = get<HTMLElement>("focus-state");
  const time = get<HTMLElement>("focus-time");
  const notice = get<HTMLElement>("focus-notice");
  const controls = {
    start: get<HTMLButtonElement>("focus-start"), pause: get<HTMLButtonElement>("focus-pause"),
    resume: get<HTMLButtonElement>("focus-resume"), endEarly: get<HTMLButtonElement>("focus-end"),
    abandon: get<HTMLButtonElement>("focus-abandon"), dismissFeedback: get<HTMLButtonElement>("focus-dismiss"),
  };
  const format = (ms: number) => { const seconds = Math.ceil(ms / 1000); return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`; };
  return {
    pending(count: number) { get<HTMLElement>("focus-pending").textContent = `当前待处理 ${count} 项提醒，可从托盘「查看待处理」打开。`; },
    render(state: FocusViewState) {
      const session = state.snapshot?.data?.session;
      const blocked = !session || !!state.snapshot?.error || !!state.snapshot?.stopped;
      if (duration.value !== String(state.durationMinutes)) duration.value = Number.isFinite(state.durationMinutes) ? String(state.durationMinutes) : "";
      duration.disabled = blocked || state.busy || !["idle", "finished"].includes(session?.status ?? "");
      const visible = {
        start: session?.status === "idle" || session?.status === "finished",
        pause: session?.status === "running",
        resume: session?.status === "paused" || session?.status === "interrupted",
        endEarly: ["running", "paused", "interrupted"].includes(session?.status ?? ""),
        abandon: ["running", "paused", "interrupted"].includes(session?.status ?? ""),
        dismissFeedback: session?.status === "finished" && session.outcome === "natural" && session.feedback === "pending",
      };
      for (const [action, button] of Object.entries(controls) as [keyof typeof controls, HTMLButtonElement][]) {
        button.hidden = !visible[action]; button.disabled = blocked || state.busy || !visible[action];
      }
      if (!state.snapshot) status.textContent = "正在读取专注状态";
      else if (state.snapshot.stopped) status.textContent = "专注服务已停止";
      else if (state.snapshot.error) status.textContent = state.snapshot.error === "loading" ? "正在读取专注状态" : ["invalidFile", "unsupportedVersion", "readOnly", "fileChanged"].includes(state.snapshot.error) ? "专注记录受保护，当前只读" : "专注记录不可用，请稍后重试";
      else if (!session) status.textContent = "正在读取专注状态";
      else if (session.status === "idle") status.textContent = "尚未开始";
      else if (session.status === "running") status.textContent = state.displayRemainingMs === 0 ? "等待专注服务确认完成" : "专注进行中";
      else if (session.status === "paused") status.textContent = "已暂停";
      else if (session.status === "interrupted") status.textContent = "计时有待确认的中断，可选择继续或结束";
      else if (session.status === "finished") status.textContent = session.outcome === "natural" ? session.feedback === "pending" ? "自然完成 · 待查看" : "自然完成" : session.outcome === "endedEarly" ? "已提前结束" : "已放弃";
      time.textContent = state.displayRemainingMs === null ? "— — : — —" : format(state.displayRemainingMs);
      notice.textContent = state.notice;
    },
    bind(actions: { setDuration(value: number): void; act(type: FocusAction): Promise<void> }) {
      const edit = () => actions.setDuration(duration.valueAsNumber);
      duration.addEventListener("input", edit);
      const listeners = (Object.entries(controls) as [FocusAction, HTMLButtonElement][]).map(([action, button]) => {
        const click = () => { void actions.act(action); }; button.addEventListener("click", click); return () => button.removeEventListener("click", click);
      });
      return () => { duration.removeEventListener("input", edit); listeners.forEach(stop => stop()); };
    },
  };
}
