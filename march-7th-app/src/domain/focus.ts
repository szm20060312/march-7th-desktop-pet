export type FocusSession =
  | { status: "idle" }
  | { status: "running"; duration_ms: number; remaining_ms: number; anchor_utc_ms: number }
  | { status: "paused" | "interrupted"; duration_ms: number; remaining_ms: number }
  | { status: "finished"; duration_ms: number; outcome: "natural" | "endedEarly" | "abandoned"; feedback: "none" | "pending" | "dismissed" };

export type FocusSnapshot = {
  revision: number;
  data: { version: 2; session: FocusSession; task: CurrentTask | null } | null;
  error: string | null;
  stopped: boolean;
};
export type FocusChange = { snapshot: FocusSnapshot; completedNow: boolean; error: string | null };
export type FocusCommand =
  | { type: "start"; durationMs: number; taskName?: string }
  | { type: "pause" | "resume" | "endEarly" | "abandon" | "dismissFeedback" | "completeTask" | "abandonTask" };

export type CurrentTask = { name: string; status: "active" | "completed" | "abandoned" };
export function normalizeTaskName(value: string): string {
  if (/[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(value)) throw Error("invalidTaskName");
  const name = value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");
  // Match Rust Unicode scalar count and UTF-8 bytes, including non-BMP characters.
  const bytes = [...name].reduce((total, c) => total + (c.codePointAt(0)! <= 0x7f ? 1 : c.codePointAt(0)! <= 0x7ff ? 2 : c.codePointAt(0)! <= 0xffff ? 3 : 4), 0);
  if ([...name].length > 80 || bytes > 256) throw Error("invalidTaskName");
  return name;
}
