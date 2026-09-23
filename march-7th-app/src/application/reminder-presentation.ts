import type { ReminderCommand, ReminderId, ReminderSnapshot } from "../domain/reminder";
export interface ReminderViewState {
  presentationId: number | null;
  rows: { id: ReminderId; done: boolean; busy: boolean }[];
  disabled: boolean;
  emptyText: string;
  error: string;
}
export function createReminderPresentation(ports: {
  render(state: ReminderViewState): void;
  command(command: ReminderCommand): Promise<ReminderSnapshot | undefined>;
  ready(presentationId: number): Promise<void>;
  reportError?(error: unknown): void;
}) {
  let snapshot: ReminderSnapshot | null = null; let order: ReminderId[] = []; let batch: number | null = null;
  let disposed = false; let error = ""; let renderToken = 0; let operation = 0;
  const busy = new Set<ReminderId>(); let allBusy = false;
  const disabled = () => !snapshot || snapshot.stopped || snapshot.persistence.status === "loading";
  const render = (acknowledge = true) => {
    if (disposed) return;
    const p = snapshot?.presentation; const token = ++renderToken;
    ports.render({ presentationId: p?.id ?? null, rows: order.map(id => ({ id, done: !p?.items.includes(id) || !snapshot?.progress.find(v => v.id === id)?.pending, busy: busy.has(id) || allBusy })), disabled: disabled() || allBusy, emptyText: !snapshot || snapshot.persistence.status === "loading" ? "正在读取提醒…" : p ? "暂无待处理提醒" : "当前没有展示中的提醒", error });
    // The view port is synchronous DOM rendering; readiness follows actual content.
    if (p && !snapshot?.stopped && acknowledge) void ports.ready(p.id).catch(cause => {
      if (disposed || token !== renderToken) return;
      ports.reportError?.(cause);
      error = "提醒窗口尚未就绪，请从托盘重新查看。"; render(false);
    });
  };
  const receive = (next: ReminderSnapshot) => {
    if (disposed || (snapshot && next.revision < snapshot.revision)) return;
    snapshot = next; const p = next.presentation;
    if ((p?.id ?? null) !== batch) { batch = p?.id ?? null; order = []; busy.clear(); allBusy = false; operation++; }
    for (const id of p?.items ?? []) if (!order.includes(id)) order.push(id);
    error = next.stopped ? "提醒服务已停止，请重启应用。" : next.runtimeError ? "提醒操作暂不可用，请稍后重试。" : ""; render();
  };
  const run = async (command: ReminderCommand, id?: ReminderId) => {
    if (disposed || disabled() || allBusy) return;
    if (id) { if (busy.has(id)) return; busy.add(id); } else allBusy = true;
    const token = operation; render();
    try { const reply = await ports.command(command); if (!disposed && reply) receive(reply); }
    catch { if (!disposed && token === operation) error = "操作未完成，请重试。"; }
    finally { if (!disposed && token === operation) { if (id) busy.delete(id); else allBusy = false; render(); } }
  };
  render();
  return {
    receive,
    async complete(id: ReminderId) { if (!snapshot?.presentation?.items.includes(id) || !snapshot.progress.find(p => p.id === id)?.pending) return; await run({ type: "complete", id }, id); },
    async snooze() { if (snapshot?.presentation) await run({ type: "snoozeAll" }); },
    async dismiss() { const id = snapshot?.presentation?.id; if (id) await run({ type: "dismiss", presentationId: id }); },
    error(message: string) { if (!disposed) { error = message; render(false); } },
    dispose() { disposed = true; renderToken++; operation++; busy.clear(); },
  };
}
