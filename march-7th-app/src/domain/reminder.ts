// Wire DTOs only. Scheduling, completion and persistence belong to Rust.
export const reminderIds = ["water", "move", "eyes"] as const;
export type ReminderId = typeof reminderIds[number];
export const reminderLabels: Record<ReminderId, string> = { water: "喝水", move: "起身活动", eyes: "休息眼睛" };
export interface ReminderSettings {
  items: { id: ReminderId; enabled: boolean; intervalMinutes: number }[];
  activeHours: { kind: "allDay" } | { kind: "daily"; start: number; end: number };
  snoozeMinutes: number;
}
export interface ReminderSnapshot {
  revision: number;
  settings: ReminderSettings;
  progress: { id: ReminderId; nextDueAt: number | null; pending: boolean; autoHandled: boolean }[];
  paused: boolean;
  quiet: { until: number; durationMinutes: number } | null;
  snoozePending: boolean;
  presentation: { id: number; mode: "automatic" | "manual"; items: ReminderId[]; closesAt: number | null; focusCompleted?: boolean; choicesOpen?: boolean } | null;
  persistence: { status: "loading" | "default" | "saved" | "unsaved" | "readOnly"; code: string | null };
  runtimeError: string | null;
  stopped: boolean;
}
export type ReminderCommand =
  | { type: "updateSettings"; settings: ReminderSettings }
  | { type: "complete"; id: ReminderId }
  | { type: "snoozeAll" }
  | { type: "setPaused"; paused: boolean }
  | { type: "showPending" }
  | { type: "dismiss"; presentationId: number };
export type ReminderResponse = { revision: number } & ({ type: "complete"; id: ReminderId } | { type: "snoozeAll" });
export function copySettings(settings: ReminderSettings): ReminderSettings {
  return { items: reminderIds.map(id => ({ ...settings.items.find(item => item.id === id)! })), activeHours: settings.activeHours.kind === "allDay" ? { kind: "allDay" } : { ...settings.activeHours }, snoozeMinutes: settings.snoozeMinutes };
}
export function sameSettings(a: ReminderSettings, b: ReminderSettings): boolean {
  return JSON.stringify(copySettings(a)) === JSON.stringify(copySettings(b));
}
