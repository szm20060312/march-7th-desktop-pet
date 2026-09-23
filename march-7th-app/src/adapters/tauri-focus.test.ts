import { expect, it, vi } from "vitest";
import { connectFocus, parseFocusChange } from "./tauri-focus";

const snapshot = (revision: number) => ({ revision, data: { version: 1, session: { status: "idle" } }, error: null, stopped: false });
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };

it("subscribes before read, ignores older replies, and never polls mutating view", async () => {
  let event!: (value: unknown) => void; let read!: (value: unknown) => void;
  const invoke = vi.fn(name => name === "get_focus" ? new Promise(yes => { read = yes; }) : Promise.resolve(snapshot(4)));
  const select = vi.fn();
  const connection = connectFocus({ select, reportError: vi.fn(), transport: { invoke, listen: async (_name, callback) => { event = callback; return () => {}; } } });
  await flush(); event({ snapshot: snapshot(3), completedNow: false, error: null }); read({ snapshot: snapshot(2), completedNow: false, error: null }); await flush();
  expect(select).toHaveBeenCalledTimes(1);
  expect(select.mock.lastCall?.[0].snapshot.revision).toBe(3);
  expect(invoke.mock.calls.map(([name]) => name)).toEqual(["get_focus"]);
  await connection.command({ type: "pause" });
  expect(invoke).toHaveBeenLastCalledWith("focus_command", { command: { type: "pause" } });
  connection.dispose();
});

it("rejects malformed native state and preserves the last accepted snapshot", () => {
  expect(() => parseFocusChange({ snapshot: { ...snapshot(2), data: { version: 1, session: { status: "running", duration_ms: 1000, remaining_ms: 500, anchor_utc_ms: 1 } } }, completedNow: false, error: null })).toThrow();
  expect(() => parseFocusChange({ snapshot: snapshot(2), completedNow: "yes", error: null })).toThrow();
});

it("accepts Rust snake_case session fields and keeps readback completion non-proactive", () => {
  const change = parseFocusChange({ snapshot: { ...snapshot(7), data: { version: 1, session: { status: "running", duration_ms: 1_500_000, remaining_ms: 450_000, anchor_utc_ms: 100 } } }, completedNow: false, error: null });
  expect(change.snapshot.data?.session).toEqual({ status: "running", duration_ms: 1_500_000, remaining_ms: 450_000, anchor_utc_ms: 100 });
  expect(change.completedNow).toBe(false);
});

it("checks a late error envelope against current read-only state before showing it", async () => {
  let event!: (value: unknown) => void;
  const current = { snapshot: snapshot(4), completedNow: false, error: null };
  const invoke = vi.fn().mockResolvedValue(current); const select = vi.fn();
  const connection = connectFocus({ select, reportError: vi.fn(), transport: { invoke, listen: async (_name, callback) => { event = callback; return () => {}; } } });
  await flush(); select.mockClear();
  event({ snapshot: snapshot(3), completedNow: false, error: "writeFailed" }); await flush();
  expect(select.mock.calls.every(([change]) => change.error === null)).toBe(true);
  expect(invoke.mock.calls.every(([name]) => name === "get_focus")).toBe(true);
  connection.dispose();
});

it("does not resurrect a delayed diagnostic read after same-revision recovery", async () => {
  let event!: (value: unknown) => void; let finish!: (value: unknown) => void;
  const initial = { snapshot: snapshot(4), completedNow: false, error: null };
  const invoke = vi.fn().mockResolvedValueOnce(initial).mockImplementation(() => new Promise(yes => { finish = yes; }));
  const select = vi.fn();
  const connection = connectFocus({ select, reportError: vi.fn(), transport: { invoke, listen: async (_name, callback) => { event = callback; return () => {}; } } });
  await flush(); select.mockClear();
  event({ snapshot: snapshot(4), completedNow: false, error: "writeFailed" }); await flush();
  event({ snapshot: snapshot(4), completedNow: false, error: null });
  finish({ snapshot: snapshot(4), completedNow: false, error: "writeFailed" }); await flush();
  expect(select.mock.calls.every(([change]) => change.error === null)).toBe(true);
  connection.dispose();
});

it("ignores a failed old read after a newer focus event succeeds", async () => {
  let event!: (value: unknown) => void; let fail!: (reason: unknown) => void;
  const invoke = vi.fn(() => new Promise((_, no) => { fail = no; }));
  const select = vi.fn(); const reportError = vi.fn();
  const connection = connectFocus({ select, reportError, transport: { invoke, listen: async (_name, callback) => { event = callback; return () => {}; } } });
  await flush(); event({ snapshot: snapshot(3), completedNow: false, error: null });
  fail(Error("old read failed")); await flush();
  expect(select.mock.lastCall?.[0].snapshot.revision).toBe(3);
  expect(reportError).not.toHaveBeenCalled();
  connection.dispose();
});

it("invalidates a pending read across close and reopen even without a focus event", async () => {
  let failOld!: (reason: unknown) => void;
  const current = { snapshot: snapshot(3), completedNow: false, error: null };
  const invoke = vi.fn().mockImplementationOnce(() => new Promise((_, no) => { failOld = no; })).mockResolvedValue(current);
  const select = vi.fn(); const reportError = vi.fn();
  const connection = connectFocus({ select, reportError, transport: { invoke, listen: async () => () => {} } });
  await flush(); connection.close(); await connection.reopen();
  expect(select.mock.lastCall?.[0]).toEqual(current);
  failOld(Error("old window")); await flush();
  expect(reportError).not.toHaveBeenCalled();
  connection.dispose();
});
