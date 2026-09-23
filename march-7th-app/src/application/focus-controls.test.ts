import { expect, it, vi } from "vitest";
import { createFocusControls } from "./focus-controls";
import type { FocusSession, FocusSnapshot } from "../domain/focus";

const snapshot = (revision: number, session: FocusSession): FocusSnapshot =>
  ({ revision, data: { version: 1, session }, error: null, stopped: false });
const idle = snapshot(1, { status: "idle" });
const running = snapshot(2, { status: "running", duration_ms: 1_500_000, remaining_ms: 1_500_000, anchor_utc_ms: 100 });
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };

it("validates duration, starts only after a committed snapshot, and blocks repeat commands", async () => {
  let resolve!: (value: FocusSnapshot) => void;
  const command = vi.fn(() => new Promise<FocusSnapshot>(yes => { resolve = yes; }));
  const render = vi.fn();
  const controls = createFocusControls({ command, render, now: () => 1000 });
  controls.receive({ snapshot: idle, completedNow: false, error: null });
  controls.setDuration(0); await controls.act("start");
  expect(command).not.toHaveBeenCalled();
  expect(render.mock.lastCall?.[0].notice).toContain("1–240");
  controls.setDuration(241); await controls.act("start");
  expect(command).not.toHaveBeenCalled();
  controls.setDuration(25); const pending = controls.act("start"); controls.act("start");
  expect(command).toHaveBeenCalledExactlyOnceWith({ type: "start", durationMs: 1_500_000 });
  expect(render.mock.lastCall?.[0].busy).toBe(true);
  resolve(running); await pending;
  expect(render.mock.lastCall?.[0].snapshot.data.session.status).toBe("running");
  controls.dispose();
});

it("keeps failed commands and late hidden responses from claiming a new state", async () => {
  let reject!: (error: unknown) => void;
  const command = vi.fn(() => new Promise<FocusSnapshot>((_, no) => { reject = no; }));
  const render = vi.fn();
  const controls = createFocusControls({ command, render, now: () => 1000 });
  controls.receive({ snapshot: running, completedNow: false, error: null });
  const pending = controls.act("pause"); controls.close(); controls.reopen();
  const paused = snapshot(3, { status: "paused", duration_ms: 1_500_000, remaining_ms: 900_000 });
  controls.receive({ snapshot: paused, completedNow: false, error: null });
  reject({ code: "writeFailed" }); await pending;
  expect(render.mock.lastCall?.[0].snapshot).toEqual(paused);
  expect(render.mock.lastCall?.[0].notice).toBe("");
  controls.dispose();
});

it("shows a current write failure, interruption and pending natural result, then dismisses only after commit", async () => {
  const command = vi.fn().mockRejectedValueOnce({ code: "saveFailed" }).mockResolvedValueOnce(snapshot(5, { status: "finished", duration_ms: 1_500_000, outcome: "natural", feedback: "dismissed" }));
  const render = vi.fn();
  const controls = createFocusControls({ command, render, now: () => 1000 });
  controls.receive({ snapshot: running, completedNow: false, error: null });
  await controls.act("pause"); expect(render.mock.lastCall?.[0].notice).toContain("未确认");
  expect(render.mock.lastCall?.[0].snapshot.data.session.status).toBe("running");
  controls.receive({ snapshot: snapshot(3, { status: "interrupted", duration_ms: 1_500_000, remaining_ms: 800_000 }), completedNow: false, error: null });
  expect(render.mock.lastCall?.[0].snapshot.data.session.status).toBe("interrupted");
  controls.receive({ snapshot: snapshot(4, { status: "finished", duration_ms: 1_500_000, outcome: "natural", feedback: "pending" }), completedNow: false, error: null });
  await controls.act("dismissFeedback");
  expect(command).toHaveBeenLastCalledWith({ type: "dismissFeedback" });
  expect(render.mock.lastCall?.[0].snapshot.data.session.feedback).toBe("dismissed");
  controls.dispose(); await flush();
});

it("visually interpolates without invoking view and waits for backend completion", () => {
  let now = 1000; const command = vi.fn(); const render = vi.fn();
  const controls = createFocusControls({ command, render, now: () => now });
  controls.receive({ snapshot: running, completedNow: false, error: null });
  now += 1_500_001; controls.tick();
  expect(render.mock.lastCall?.[0].displayRemainingMs).toBe(0);
  expect(render.mock.lastCall?.[0].snapshot.data.session.status).toBe("running");
  expect(command).not.toHaveBeenCalled(); controls.dispose();
});

it("clears a recovered background error even when the durable revision stays the same", () => {
  const render = vi.fn();
  const controls = createFocusControls({ command: vi.fn(), render, now: () => 1000 });
  controls.receive({ snapshot: running, completedNow: false, error: "writeFailed" });
  expect(render.mock.lastCall?.[0].notice).toContain("未写入");
  controls.receive({ snapshot: running, completedNow: false, error: null });
  expect(render.mock.lastCall?.[0].notice).toBe("");
  controls.dispose();
});

it("compensates a running snapshot read 50 seconds after its Rust clock anchor", () => {
  const render = vi.fn();
  const controls = createFocusControls({ command: vi.fn(), render, now: () => 150_000 });
  controls.receive({ snapshot: snapshot(9, { status: "running", duration_ms: 60_000, remaining_ms: 60_000, anchor_utc_ms: 100_000 }), completedNow: false, error: null });
  expect(render.mock.lastCall?.[0].displayRemainingMs).toBe(10_000);
  controls.dispose();
});

it("uses monotonic visual progress after receipt and clamps small wall-clock skew and zero", () => {
  let wall = 150_000; let monotonic = 1000; const render = vi.fn();
  const controls = createFocusControls({ command: vi.fn(), render, now: () => wall, monotonicNow: () => monotonic });
  controls.receive({ snapshot: snapshot(9, { status: "running", duration_ms: 60_000, remaining_ms: 60_000, anchor_utc_ms: 100_000 }), completedNow: false, error: null });
  expect(render.mock.lastCall?.[0].displayRemainingMs).toBe(10_000);
  wall = 149_500; monotonic += 1000; controls.tick();
  expect(render.mock.lastCall?.[0].displayRemainingMs).toBe(9000);
  wall = 200_000; monotonic += 10_000; controls.tick();
  expect(render.mock.lastCall?.[0].displayRemainingMs).toBe(0);
  expect(render.mock.lastCall?.[0].snapshot.data.session.status).toBe("running");
  wall = 99_500;
  controls.receive({ snapshot: snapshot(10, { status: "running", duration_ms: 60_000, remaining_ms: 60_000, anchor_utc_ms: 100_000 }), completedNow: false, error: null });
  expect(render.mock.lastCall?.[0].displayRemainingMs).toBe(60_000);
  controls.dispose();
});

it("clears a recovered read error on the same durable revision", () => {
  const render = vi.fn();
  const controls = createFocusControls({ command: vi.fn(), render, now: () => 1000 });
  controls.receive({ snapshot: idle, completedNow: false, error: null });
  controls.error({ code: "workerUnavailable" });
  expect(render.mock.lastCall?.[0].notice).toContain("暂不可用");
  controls.receive({ snapshot: idle, completedNow: false, error: null });
  expect(render.mock.lastCall?.[0].notice).toBe("");
  controls.dispose();
});
