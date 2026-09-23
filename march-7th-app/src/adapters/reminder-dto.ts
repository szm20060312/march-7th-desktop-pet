import { reminderIds, type ReminderId, type ReminderResponse, type ReminderSettings, type ReminderSnapshot } from "../domain/reminder";
function invalid(): never { throw new Error("Invalid reminder DTO"); }
function record(value: unknown): Record<string, unknown> { if (!value || typeof value !== "object" || Array.isArray(value)) return invalid(); return value as Record<string, unknown>; }
function integer(value: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): number { if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) return invalid(); return value; }
function bool(value: unknown): boolean { if (typeof value !== "boolean") return invalid(); return value; }
function code(value: unknown): string | null { if (value === null) return null; if (typeof value !== "string" || !value.length || value.length > 128) return invalid(); return value; }
function enumeration<T extends string>(value: unknown, options: readonly T[]): T { if (typeof value !== "string" || !options.includes(value as T)) return invalid(); return value as T; }
const id = (value: unknown): ReminderId => enumeration(value, reminderIds);
const utc = (value: unknown): number => integer(value, 0, 253402300799999);
function nullableUtc(value: unknown): number | null { return value === null ? null : utc(value); }
function identities<T extends { id: ReminderId }>(value: unknown, parse: (item: Record<string, unknown>) => T): T[] {
  if (!Array.isArray(value) || value.length !== 3) return invalid();
  const items = value.map(v => parse(record(v)));
  if (new Set(items.map(v => v.id)).size !== 3) return invalid();
  return reminderIds.map(key => items.find(v => v.id === key)!);
}
export function parseReminderSettings(value: unknown): ReminderSettings {
  const raw = record(value); const hours = record(raw.activeHours); const kind = enumeration(hours.kind, ["allDay", "daily"]);
  const activeHours: ReminderSettings["activeHours"] = kind === "allDay" ? { kind } : { kind, start: integer(hours.start, 0, 1439), end: integer(hours.end, 0, 1439) };
  if (activeHours.kind === "daily" && activeHours.start === activeHours.end) return invalid();
  return { items: identities(raw.items, v => ({ id: id(v.id), enabled: bool(v.enabled), intervalMinutes: integer(v.intervalMinutes, 1, 1440) })), activeHours, snoozeMinutes: integer(raw.snoozeMinutes, 1, 120) };
}
export function parseReminderSnapshot(value: unknown): ReminderSnapshot {
  const raw = record(value); const persistence = record(raw.persistence);
  let presentation: ReminderSnapshot["presentation"] = null;
  if (raw.presentation !== null) {
    const p = record(raw.presentation);
    if (!Array.isArray(p.items) || p.items.length > 3) return invalid();
    const items = p.items.map(id); if (new Set(items).size !== items.length) return invalid();
    presentation = { id: integer(p.id, 1), mode: enumeration(p.mode, ["automatic", "manual"]), items, closesAt: nullableUtc(p.closesAt) };
  }
  const q = raw.quiet === null ? null : record(raw.quiet);
  return {
    revision: integer(raw.revision), settings: parseReminderSettings(raw.settings),
    progress: identities(raw.progress, v => ({ id: id(v.id), nextDueAt: nullableUtc(v.nextDueAt), pending: bool(v.pending), autoHandled: bool(v.autoHandled) })),
    paused: bool(raw.paused), quiet: q === null ? null : { until: utc(q.until), durationMinutes: integer(q.durationMinutes, 1, 120) }, snoozePending: bool(raw.snoozePending), presentation,
    persistence: { status: enumeration(persistence.status, ["loading", "default", "saved", "unsaved", "readOnly"]), code: code(persistence.code) }, runtimeError: code(raw.runtimeError), stopped: bool(raw.stopped),
  };
}
export function parseReminderResponse(value: unknown): ReminderResponse {
  const raw = record(value); const revision = integer(raw.revision); const type = enumeration(raw.type, ["complete", "snoozeAll"]);
  return type === "complete" ? { revision, type, id: id(raw.id) } : { revision, type };
}
