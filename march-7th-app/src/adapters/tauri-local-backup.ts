import { invoke } from "@tauri-apps/api/core";
import type { BackupResult, SelectedBackup } from "../application/local-backup";
const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const minute = (value: unknown) => Number.isSafeInteger(value) && (value as number) >= 0 && (value as number) <= 1440;
function validPreview(value: unknown): boolean {
  if (!record(value) || !Number.isSafeInteger(value.createdAtUtcMs) || typeof value.selectedCharacterId !== "string" || typeof value.hasDesktopPlacement !== "boolean" || !record(value.reminders) || !record(value.focus)) return false;
  if (!["idle", "running", "paused", "interrupted", "finished"].includes(String(value.focus.status)) || typeof value.focus.defaultedFromV1 !== "boolean" || Object.keys(value.focus).length !== 2) return false;
  if (value.focus.defaultedFromV1 && value.focus.status !== "idle") return false;
  const reminder = value.reminders;
  if (!Array.isArray(reminder.items) || reminder.items.length !== 3 || !record(reminder.activeHours) || !minute(reminder.snoozeMinutes) || !Number.isSafeInteger(reminder.pendingCount) || typeof reminder.paused !== "boolean" || typeof reminder.snoozePending !== "boolean") return false;
  if (reminder.quietUntilUtcMs !== null && !Number.isSafeInteger(reminder.quietUntilUtcMs)) return false;
  if (reminder.activeHours.kind !== "allDay" && !(reminder.activeHours.kind === "daily" && minute(reminder.activeHours.start) && minute(reminder.activeHours.end))) return false;
  const ids = new Set<string>();
  for (const item of reminder.items) {
    if (!record(item) || typeof item.id !== "string" || !["water", "move", "eyes"].includes(item.id) || ids.has(item.id) || typeof item.enabled !== "boolean" || !minute(item.intervalMinutes)) return false;
    ids.add(item.id);
  }
  return true;
}

export async function exportLocalBackup(): Promise<BackupResult> {
  const result: unknown = await invoke("export_local_backup");
  if (result === "saved" || result === "cancelled") return result;
  throw { code: "backupInvalidResponse" };
}

export async function selectLocalBackup(): Promise<SelectedBackup | null> {
  const result: unknown = await invoke("select_local_backup");
  if (result === null) return null;
  if (!record(result)) throw { code: "backupInvalidResponse" };
  const { ticket, preview } = result;
  if (!Number.isSafeInteger(ticket) || (ticket as number) <= 0 || !validPreview(preview)) throw { code: "backupInvalidResponse" };
  return result as SelectedBackup;
}

export async function confirmLocalBackup(ticket: number): Promise<{ restartRequired: boolean }> {
  const result: unknown = await invoke("confirm_local_backup", { ticket });
  if (typeof result === "object" && result !== null && "restartRequired" in result && result.restartRequired === true) return { restartRequired: true };
  throw { code: "backupInvalidResponse" };
}

export async function cancelLocalBackup(ticket: number): Promise<void> {
  await invoke("cancel_local_backup", { ticket });
}
