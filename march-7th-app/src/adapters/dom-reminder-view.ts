import type { ReminderViewState } from "../application/reminder-presentation";
import { reminderLabels, type ReminderId } from "../domain/reminder";
export function createDomReminderView(root: Document) {
  const element = (id: string) => { const el = root.getElementById(id); if (!el) throw new Error(`Missing reminder element ${id}`); return el; };
  const rows = element("reminder-rows"); const empty = element("reminder-empty"); const error = element("reminder-error");
  const snooze = element("snooze-reminder") as HTMLButtonElement; const dismiss = element("dismiss-reminder") as HTMLButtonElement;
  let batch: number | null = null; const nodes = new Map<ReminderId, { row: HTMLElement; button: HTMLButtonElement }>();
  let actions: { complete(id: ReminderId): Promise<void>; snooze(): Promise<void>; dismiss(): Promise<void> } | undefined;
  return {
    render(state: ReminderViewState) {
      if (batch !== state.presentationId) { rows.replaceChildren(); nodes.clear(); batch = state.presentationId; }
      for (const item of state.rows) {
        let node = nodes.get(item.id);
        if (!node) {
          const row = root.createElement("div"); row.className = "reminder-row";
          const label = root.createElement("span"); label.textContent = reminderLabels[item.id];
          const button = root.createElement("button"); button.type = "button"; button.setAttribute("aria-label", `${reminderLabels[item.id]}，完成`);
          button.addEventListener("click", () => { void actions?.complete(item.id); }); row.append(label, button); rows.append(row); node = { row, button }; nodes.set(item.id, node);
        }
        node.row.dataset.done = String(item.done); node.button.textContent = item.done ? "已处理" : item.busy ? "处理中" : "完成"; node.button.disabled = item.done || item.busy || state.disabled;
      }
      empty.textContent = state.emptyText; empty.hidden = state.rows.length > 0;
      error.textContent = state.error;
      snooze.disabled = dismiss.disabled = state.presentationId === null || state.disabled;
    },
    bind(value: NonNullable<typeof actions>) {
      actions = value; const later = () => { void actions?.snooze(); }; const hide = () => { void actions?.dismiss(); };
      snooze.addEventListener("click", later); dismiss.addEventListener("click", hide);
      return () => { actions = undefined; snooze.removeEventListener("click", later); dismiss.removeEventListener("click", hide); nodes.clear(); rows.replaceChildren(); };
    },
  };
}
