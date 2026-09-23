import { describe, expect, it, vi } from "vitest";
import { march7th } from "../characters/march-7th";
import type { CursorSample } from "../domain/animation";
import type { PetGestureHandlers, PetView, Scheduler } from "./ports";
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
  let gestureHandlers!: PetGestureHandlers;
  let now = 0;
  let id = 0;
  const delays = new Map<number, { callback: () => void; ms: number }>();
  const frames = new Map<number, (now: number) => void>();
  const scheduler: Scheduler = {
    now: () => now,
    requestFrame: fn => { frames.set(++id, fn); return id; },
    cancelFrame: id => { frames.delete(id); },
    setDelay: (fn, ms) => { delays.set(++id, { callback: fn, ms }); return id; },
    cancelDelay: id => { delays.delete(id); },
  };
  const unsubscribe = vi.fn();
  const view: PetView = {
    configure: vi.fn(), center: () => ({ x: 0, y: 0 }), render: vi.fn(),
    setTracking: vi.fn(), showPhrase: vi.fn(), clearPhrase: vi.fn(), setPresentationError: vi.fn(),
  };
  const gestures = { subscribe: vi.fn((handlers: PetGestureHandlers) => { gestureHandlers = handlers; return unsubscribe; }) };
  const host = { sampleCursor: vi.fn().mockResolvedValue(cursor), startDragging: vi.fn().mockResolvedValue(undefined) };
  const reportError = vi.fn();
  return { view, host, scheduler, gestures, reportError, delays, frames, unsubscribe,
    setNow: (value: number) => { now = value; },
    gesture: (name: keyof PetGestureHandlers) => gestureHandlers[name](),
    start: () => startPetRuntime({ character: march7th, view, gestures, host, scheduler, reportError }),
  };
}

describe("pet runtime lifecycle", () => {
  it("does not overlap slow IPC and waits until it completes before scheduling", async () => {
    const h = harness();
    const pending = deferred<CursorSample>();
    h.host.sampleCursor.mockReturnValue(pending.promise);
    const runtime = h.start();
    expect(h.delays.size).toBe(0);
    pending.resolve(cursor);
    await flush();
    expect(h.view.setTracking).toHaveBeenLastCalledWith("active");
    expect(h.delays.size).toBe(1);
    runtime.stop();
    expect(h.delays.size).toBe(0);
    expect(h.frames.size).toBe(0);
  });
  it.each(["resolve", "reject"] as const)("ignores a late %s after stop", async outcome => {
    const h = harness();
    const pending = deferred<CursorSample>();
    h.host.sampleCursor.mockReturnValue(pending.promise);
    const runtime = h.start();
    const queuedFrame = [...h.frames.values()][0];
    runtime.stop();
    runtime.stop();
    if (outcome === "resolve") pending.resolve(cursor);
    else pending.reject(new Error("closed"));
    await flush();
    queuedFrame(0);
    h.gesture("drag");
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
    const runtime = h.start();
    await flush();
    expect(h.view.setTracking).toHaveBeenLastCalledWith("unavailable");
    const [id, next] = [...h.delays].find(([, delay]) => delay.ms === 33)!;
    h.delays.delete(id);
    next.callback();
    await flush();
    expect(h.view.setTracking).toHaveBeenLastCalledWith("active");
    runtime.stop();
  });
  it("captures native drag rejection without ending the runtime", async () => {
    const h = harness();
    const error = new Error("drag not supported");
    h.host.startDragging.mockRejectedValueOnce(error);
    const runtime = h.start();
    h.gesture("drag");
    await flush();
    expect(h.reportError).toHaveBeenCalledWith("drag", error);
    expect(h.frames.size).toBe(1);
    runtime.stop();
  });
  it("maps responses to one-shot actions and rotates phrases in order", () => {
    const h = harness();
    const runtime = h.start();
    expect(runtime.respond("click")).toBe(true);
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("咱在呢！");
    const paint = [...h.frames.values()][0];
    paint(0);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: 3, column: 0 });
    expect(runtime.respond("click")).toBe(true);
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("今天也一起加油吧。");
    expect([...h.delays.values()].filter(delay => delay.ms === 3000)).toHaveLength(1);
    runtime.respond("doubleClick");
    paint(0);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: 4, column: 0 });
    runtime.stop();
  });
  it("holds the offered cup after the four frames until an authoritative reminder end", () => {
    const h = harness(); const runtime = h.start();
    expect(runtime.respond("reminderDue", "先喝口水吧，咱们再继续！", "offerWater")).toBe(true);
    const paint = [...h.frames.values()][0]; paint(0);
    expect(h.view.render).toHaveBeenLastCalledWith({ asset: "waterOffer", row: 0, column: 0 });
    paint(10_000);
    expect(h.view.render).toHaveBeenLastCalledWith({ asset: "waterOffer", row: 1, column: 1 });
    runtime.endReminder(); paint(10_001);
    expect(h.view.render).toHaveBeenLastCalledWith(expect.not.objectContaining({ asset: "waterOffer" }));
    runtime.stop();
  });
  it("clears phrases after three seconds and immediately on drag or stop", () => {
    const h = harness();
    const runtime = h.start();
    runtime.respond("reminderCompleted");
    const phraseDelay = [...h.delays.values()].find(delay => delay.ms === 3000)!;
    phraseDelay.callback();
    expect(h.view.clearPhrase).toHaveBeenCalledOnce();
    runtime.respond("reminderSnoozed");
    h.gesture("drag");
    expect(h.view.clearPhrase).toHaveBeenCalledTimes(2);
    h.setNow(200);
    runtime.respond("click");
    runtime.stop();
    expect(h.view.clearPhrase).toHaveBeenCalledTimes(3);
  });
  it("does not queue an interaction while moving", () => {
    const h = harness();
    const runtime = h.start();
    h.gesture("drag");
    expect(runtime.respond("click")).toBe(false);
    expect(h.view.showPhrase).not.toHaveBeenCalled();
    runtime.stop();
  });
  it("routes gesture clicks through the explicit response entry", () => {
    const h = harness();
    const runtime = h.start();
    h.gesture("click");
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("咱在呢！");
    h.gesture("doubleClick");
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("嘿，精神满满！");
    runtime.stop();
  });
  it("lets a live reminder consume the pet click before the ordinary click phrase", () => {
    const h = harness(); const onPetClick = vi.fn().mockReturnValue(true);
    const runtime = startPetRuntime({ character: march7th, view: h.view, gestures: h.gestures, host: h.host, scheduler: h.scheduler, reportError: h.reportError, onPetClick });
    h.gesture("click"); expect(onPetClick).toHaveBeenCalledOnce(); expect(h.view.showPhrase).not.toHaveBeenCalled();
    runtime.stop();
  });
  it("keeps the action but shows no text when a context has no phrases", () => {
    const h = harness();
    const character = { ...march7th, phrases: { ...march7th.phrases, click: undefined } };
    const runtime = startPetRuntime({ character, view: h.view, gestures: h.gestures, host: h.host, scheduler: h.scheduler, reportError: h.reportError });
    expect(runtime.respond("click")).toBe(true);
    expect(h.view.showPhrase).not.toHaveBeenCalled();
    runtime.stop();
  });
  it("clears existing text and its timer when the next accepted context has no phrase", () => {
    const h = harness();
    const character = { ...march7th, phrases: { ...march7th.phrases, doubleClick: undefined } };
    const runtime = startPetRuntime({ character, view: h.view, gestures: h.gestures, host: h.host, scheduler: h.scheduler, reportError: h.reportError });
    expect(runtime.respond("click")).toBe(true);
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("咱在呢！");
    expect([...h.delays.values()].filter(delay => delay.ms === 3_000)).toHaveLength(1);
    expect(runtime.respond("doubleClick")).toBe(true);
    expect(h.view.clearPhrase).toHaveBeenCalledOnce();
    expect([...h.delays.values()].filter(delay => delay.ms === 3_000)).toHaveLength(0);
    runtime.stop();
  });
});
