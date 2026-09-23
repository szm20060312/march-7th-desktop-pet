export type FocusSession =
  | { status: "idle" }
  | { status: "running"; duration_ms: number; remaining_ms: number; anchor_utc_ms: number }
  | { status: "paused" | "interrupted"; duration_ms: number; remaining_ms: number }
  | { status: "finished"; duration_ms: number; outcome: "natural" | "endedEarly" | "abandoned"; feedback: "none" | "pending" | "dismissed" };

export type FocusSnapshot = {
  revision: number;
  data: { version: 1; session: FocusSession } | null;
  error: string | null;
  stopped: boolean;
};
export type FocusChange = { snapshot: FocusSnapshot; completedNow: boolean; error: string | null };
export type FocusCommand =
  | { type: "start"; durationMs: number }
  | { type: "pause" | "resume" | "endEarly" | "abandon" | "dismissFeedback" };
