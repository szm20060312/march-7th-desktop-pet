import { describe, expect, it, vi } from "vitest";
import { march7th } from "../characters/march-7th";
import type { CursorSample } from "../domain/animation";
import type { PetView, Scheduler } from "./ports";
import { startPetRuntime } from "./pet-runtime";

const flush = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };
const cursor = { x: 100, y: 0, windowX: 0, windowY: 0 };
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function harness() {
  let drag!: () => void;
  let id = 0;
  const delays = new Map<number, () => void>();
  const frames = new Map<number, (now: number) => void>();
  const scheduler: Scheduler = {
    now: () => 0,
    requestFrame: fn => { frames.set(++id, fn); return id; },
    cancelFrame: id => { frames.delete(id); },
    setDelay: (fn, ms) => { expect(ms).toBe(33); delays.set(++id, fn); return id; },
    cancelDelay: id => { delays.delete(id); },
  };
  const unsubscribe = vi.fn();
  const view: PetView = {
    configure: vi.fn(), center: () => ({ x: 0, y: 0 }), render: vi.fn(),
    setTracking: vi.fn(), onDragStart: fn => { drag = fn; return unsubscribe; },
  };
  const host = { sampleCursor: vi.fn().mockResolvedValue(cursor), startDragging: vi.fn().mockResolvedValue(undefined) };
  const reportError = vi.fn();
  return { view, host, scheduler, reportError, delays, frames, unsubscribe,
    drag: () => drag(),
    start: () => startPetRuntime({ character: march7th, view, host, scheduler, reportError }),
  };
}

describe("pet runtime lifecycle", () => {
  it("does not overlap slow IPC and waits until it completes before scheduling", async () => {
    const h = harness();
    const pending = deferred<CursorSample>();
    h.host.sampleCursor.mockReturnValue(pending.promise);
    const stop = h.start();
    expect(h.delays.size).toBe(0);
    pending.resolve(cursor);
    await flush();
    expect(h.view.setTracking).toHaveBeenLastCalledWith("active");
    expect(h.delays.size).toBe(1);
    stop();
    expect(h.delays.size).toBe(0);
    expect(h.frames.size).toBe(0);
  });
  it.each(["resolve", "reject"] as const)("ignores a late %s after stop", async outcome => {
    const h = harness();
    const pending = deferred<CursorSample>();
    h.host.sampleCursor.mockReturnValue(pending.promise);
    const stop = h.start();
    const queuedFrame = [...h.frames.values()][0];
    stop();
    stop();
    if (outcome === "resolve") pending.resolve(cursor);
    else pending.reject(new Error("closed"));
    await flush();
    queuedFrame(0);
    h.drag();
    expect(h.view.setTracking).not.toHaveBeenCalled();
    expect(h.view.render).toHaveBeenCalledTimes(1); // initial frame only
    expect(h.host.startDragging).not.toHaveBeenCalled();
    expect(h.delays.size).toBe(0);
    expect(h.frames.size).toBe(0);
    expect(h.unsubscribe).toHaveBeenCalledOnce();
  });
  it("handles a failed sample and recovers on the next poll", async () => {
    const h = harness();
    h.host.sampleCursor.mockRejectedValueOnce(new Error("temporary"));
    const stop = h.start();
    await flush();
    expect(h.view.setTracking).toHaveBeenLastCalledWith("unavailable");
    const [id, next] = [...h.delays][0];
    h.delays.delete(id);
    next();
    await flush();
    expect(h.view.setTracking).toHaveBeenLastCalledWith("active");
    stop();
  });
  it("captures native drag rejection without ending the runtime", async () => {
    const h = harness();
    const error = new Error("drag not supported");
    h.host.startDragging.mockRejectedValueOnce(error);
    const stop = h.start();
    h.drag();
    await flush();
    expect(h.reportError).toHaveBeenCalledWith("drag", error);
    expect(h.frames.size).toBe(1);
    stop();
  });
});
