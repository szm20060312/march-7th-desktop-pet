import type { ReminderViewState } from "../application/reminder-presentation";
import type { CharacterDefinition } from "../domain/character";
import { reminderPrompt } from "../domain/reminder-prompt";
import { reminderLabels, type ReminderId } from "../domain/reminder";
export function createDomReminderView(root: Document) {
  const element = (id: string) => { const el = root.getElementById(id); if (!el) throw new Error(`Missing reminder element ${id}`); return el; };
  const rows = element("reminder-rows"); const empty = element("reminder-empty"); const error = element("reminder-error");
  const snooze = element("snooze-reminder") as HTMLButtonElement; const dismiss = element("dismiss-reminder") as HTMLButtonElement;
  const card = element("reminder-card"); const speaker = element("reminder-speaker"); const prompt = element("reminder-prompt");
  const avatar = element("reminder-avatar") as HTMLElement;
  const choices = element("reminder-choice-strip"); const choicePrimary = element("choice-primary") as HTMLButtonElement;
  const choiceLater = element("choice-later") as HTMLButtonElement; const choiceError = element("choice-error");
  let batch: number | null = null; const nodes = new Map<ReminderId, { row: HTMLElement; button: HTMLButtonElement }>();
  let actions: { complete(id: ReminderId): Promise<void>; snooze(): Promise<void>; dismiss(): Promise<void>; showPending?(): Promise<void> } | undefined;
  let character: CharacterDefinition | null = null; let current: ReminderViewState | null = null;
  let choiceItems: ReminderId[] = [];
  const paintVoice = () => {
    speaker.textContent = character?.displayName ?? "桌面伙伴";
    card.dataset.character = character?.id ?? "neutral";
    choices.dataset.character = character?.id ?? "neutral";
    avatar.style.backgroundImage = character ? `url("${character.atlas.src}")` : "none";
    if (!current || current.presentationId === null) { prompt.textContent = "正在读取提醒…"; return; }
    if (current.focusCompleted) {
      prompt.textContent = character?.phrases.focusCompleted?.[(current.presentationId - 1) % 2] ?? "这段专注完成了，休息一下。";
    } else if (current.mode === "automatic" && current.rows.length > 0) {
      const items = current.rows.map(row => row.id);
      prompt.textContent = character ? reminderPrompt(character, items, current.presentationId) ?? "有件小事到点了。" : "有件小事到点了。";
    } else {
      prompt.textContent = "待处理提醒";
    }
  };
  return {
    setCharacter(next: CharacterDefinition | null) { character = next; paintVoice(); },
    render(state: ReminderViewState) {
      current = state; paintVoice();
      const automatic = state.mode === "automatic" && !state.focusCompleted;
      card.hidden = automatic;
      choices.hidden = !automatic || state.choicesOpen !== true;
      choiceItems = state.rows.filter(row => !row.done).map(row => row.id);
      const choiceLabels: Record<ReminderId, string> = { water: "接过水杯", move: "活动好了", eyes: "眼睛歇好了" };
      choicePrimary.textContent = choiceItems.length === 1 ? choiceLabels[choiceItems[0]] : "查看待处理";
      choicePrimary.disabled = choices.hidden || state.disabled || choiceItems.length === 0 || state.rows.some(row => row.busy);
      choiceLater.disabled = choices.hidden || state.disabled;
      choiceError.textContent = state.error;
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
      const focus = state.focusCompleted === true;
      const title = root.getElementById("reminder-title"); if (title) title.textContent = focus ? "这段专注完成了，休息一下" : state.mode === "manual" ? "待处理事项" : "日常提醒";
      empty.textContent = focus ? `有 ${state.pendingCount ?? 0} 项待处理提醒。完成记录可从托盘「当前专注」查看。` : state.emptyText;
      snooze.textContent = focus ? "查看待处理" : "稍后提醒"; empty.hidden = state.rows.length > 0;
      error.textContent = state.error;
      snooze.disabled = dismiss.disabled = state.presentationId === null || state.disabled;
    },
    bind(value: NonNullable<typeof actions>) {
      actions = value; const later = () => { void actions?.snooze(); }; const hide = () => { void actions?.dismiss(); };
      const choose = () => { if (!current?.choicesOpen || current.mode !== "automatic") return; if (choiceItems.length === 1) void actions?.complete(choiceItems[0]); else if (choiceItems.length > 1) void actions?.showPending?.(); };
      const chooseLater = () => { if (current?.choicesOpen && current.mode === "automatic") void actions?.snooze(); };
      snooze.addEventListener("click", later); dismiss.addEventListener("click", hide);
      choicePrimary.addEventListener("click", choose); choiceLater.addEventListener("click", chooseLater);
      return () => { actions = undefined; snooze.removeEventListener("click", later); dismiss.removeEventListener("click", hide); choicePrimary.removeEventListener("click", choose); choiceLater.removeEventListener("click", chooseLater); nodes.clear(); rows.replaceChildren(); };
    },
  };
}
