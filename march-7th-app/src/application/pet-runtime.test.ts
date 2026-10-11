import { describe, expect, it, vi } from "vitest";
import { march7th } from "../characters/march-7th";
import type { CursorSample, HorizontalDirection } from "../domain/animation";
import type { PetGestureHandlers, PetView, Scheduler } from "./ports";
import { startPetRuntime } from "./pet-runtime";
import type { CharacterDefinition } from "../domain/character";
import { connectReminderResponses, type ReminderTransport } from "../adapters/tauri-reminders";

const sequencedCharacter: CharacterDefinition = {
  ...march7th,
  clips: { ...march7th.clips, wave: { row: 3, frameCount: 8, frameIntervalMs: 100 }, jump: { row: 4, frameCount: 8, frameIntervalMs: 90 } },
  waterOffer: { kind: "mainAtlas", clip: { row: 5, frameCount: 8, frameIntervalMs: 140 } },
  presentation: { neutralFrame: { row: 0, column: 0 }, neutralHoldMs: 80,
    movementTransitions: { right: { row: 6, frameCount: 4, frameIntervalMs: 60 }, left: { row: 7, frameCount: 4, frameIntervalMs: 60 } } },
};

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
    gesture: (name: keyof PetGestureHandlers, hint?: HorizontalDirection) => gestureHandlers[name](hint),
    start: (character: CharacterDefinition = march7th) => startPetRuntime({ character, view, gestures, host, scheduler, reportError }),
  };
}

describe("pet runtime lifecycle", () => {
  it("uses the drag hint before native cursor/drag promises settle and starts native dragging immediately", async () => {
    const h = harness(); const pendingCursor = deferred<CursorSample>(); const pendingDrag = deferred<void>();
    h.host.sampleCursor.mockReturnValue(pendingCursor.promise);
    h.host.startDragging.mockReturnValue(pendingDrag.promise);
    const runtime = h.start(sequencedCharacter);
    h.gesture("drag", "left");
    expect(h.host.startDragging).toHaveBeenCalledOnce();
    const draw = [...h.frames.values()][0];
    h.setNow(16); draw(16);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: 7, column: 0 });
    runtime.stop();
    pendingDrag.resolve(undefined); pendingCursor.resolve(cursor); await flush();
    expect(h.delays.size).toBe(0);
  });

  it.each(["complete", "snoozeAll"] as const)("accepts a later distinct manual %s after cup feedback ends while transport deduplicates revisions", async type => {
    const h = harness(); const runtime = h.start(sequencedCharacter);
    const events = new Map<string, (value: unknown) => void>();
    const transport: ReminderTransport = { invoke: vi.fn(), listen: async (name, callback) => { events.set(name, callback); return () => { events.delete(name); }; } };
    const accepted: boolean[] = [];
    const disconnect = connectReminderResponses({ transport, reportError: h.reportError,
      respond: response => { accepted.push(runtime.respond(response.type === "complete" ? "reminderCompleted" : "reminderSnoozed")); } });
    await flush();
    const emit = events.get("reminder-response")!;
    const paint = (now: number) => { h.setNow(now); [...h.frames.values()][0](now); };
    runtime.respond("reminderDue", "喝口水", "offerWater");
    paint(280); runtime.endReminder();
    emit({ revision: 40, type: "complete", id: "water" });
    emit({ revision: 40, type: "complete", id: "water" });
    paint(780); paint(1580);
    emit(type === "complete" ? { revision: 39, type, id: "eyes" } : { revision: 39, type });
    emit(type === "complete" ? { revision: 39, type, id: "eyes" } : { revision: 39, type });
    paint(1580);
    expect(accepted).toEqual([true, true]);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: type === "complete" ? 4 : 3, column: 0 });
    expect(vi.mocked(h.view.showPhrase).mock.calls).toEqual([["喝口水"], ["好耶，先歇一小会儿！"], [type === "complete" ? "做完啦，咱慢慢来！" : "收到，咱晚点再说！"]]);
    disconnect(); runtime.stop();
  });
  it.each([march7th, sequencedCharacter])("clears a non-water reminder after its native end for $id", character => {
    const h = harness(); const runtime = h.start(character);
    runtime.respond("reminderDue", "起来走走");
    runtime.endReminder();
    const delayedClear = [...h.delays.values()].find(delay => delay.ms > 33)!;
    expect(delayedClear).toBeDefined();
    delayedClear.callback();
    expect(h.view.clearPhrase).toHaveBeenCalledOnce();
    runtime.stop();
  });
  it("does not show stale reminder text when a non-water reminder ends before its queued action starts", () => {
    const h = harness(); const runtime = h.start(sequencedCharacter);
    runtime.respond("click");
    h.setNow(100); runtime.respond("reminderDue", "起来走走");
    h.setNow(200); runtime.endReminder(); runtime.endReminder();
    h.setNow(880); [...h.frames.values()][0](880);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: 3, column: 0 });
    expect(vi.mocked(h.view.showPhrase).mock.calls).toEqual([["咱在呢！"]]);
    runtime.stop();
  });
  it.each(["endFirst", "responseFirst"])("returns the main-atlas cup once before transport-deduplicated feedback with %s event order", async order => {
    const h = harness(); const runtime = h.start(sequencedCharacter);
    const events = new Map<string, (value: unknown) => void>();
    const accepted: boolean[] = [];
    const transport: ReminderTransport = { invoke: vi.fn(), listen: async (name, callback) => { events.set(name, callback); return () => { events.delete(name); }; } };
    const disconnect = connectReminderResponses({ transport, reportError: h.reportError,
      respond: () => { accepted.push(runtime.respond("reminderCompleted")); } });
    await flush();
    const emit = () => events.get("reminder-response")!({ revision: 40, type: "complete", id: "water" });
    const paint = (now: number) => { h.setNow(now); [...h.frames.values()][0](now); };
    runtime.respond("reminderDue", "喝口水", "offerWater");
    paint(280);
    if (order === "endFirst") runtime.endReminder();
    emit();
    expect(accepted).toEqual([true]);
    if (order === "responseFirst") runtime.endReminder();
    runtime.endReminder();
    emit();
    expect(accepted).toEqual([true]);
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("喝口水");
    paint(280); expect(h.view.render).toHaveBeenLastCalledWith({ row: 5, column: 2 });
    paint(700); expect(h.view.render).toHaveBeenLastCalledWith({ row: 0, column: 0 });
    paint(780); expect(h.view.render).toHaveBeenLastCalledWith({ row: 4, column: 0 });
    expect(vi.mocked(h.view.showPhrase).mock.calls).toEqual([["喝口水"], ["好耶，先歇一小会儿！"]]);
    const oldReturn = [...h.delays.values()].find(delay => delay.ms === 500);
    oldReturn?.callback();
    expect(h.view.clearPhrase).not.toHaveBeenCalled();
    paint(1580);
    emit();
    expect(accepted).toEqual([true]);
    expect(vi.mocked(h.view.showPhrase).mock.calls).toHaveLength(2);
    disconnect(); runtime.stop();
  });
  it("shows only the latest queued phrase when its action starts and initiates native drag without waiting", () => {
    const h = harness(); const runtime = h.start(sequencedCharacter);
    runtime.respond("click");
    h.setNow(100); runtime.respond("click");
    h.setNow(200); runtime.respond("doubleClick");
    expect(vi.mocked(h.view.showPhrase).mock.calls).toEqual([["咱在呢！"]]);
    h.setNow(880); [...h.frames.values()][0](880);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: 4, column: 0 });
    expect(vi.mocked(h.view.showPhrase).mock.calls).toEqual([["咱在呢！"], ["嘿，精神满满！"]]);
    h.setNow(1000); runtime.respond("click");
    h.gesture("drag");
    expect(h.host.startDragging).toHaveBeenCalledOnce();
    [...h.frames.values()][0](1000);
    expect(h.view.render).toHaveBeenLastCalledWith({ row: 4, column: 1 });
    const queued = [...h.frames.values()][0];
    runtime.stop();
    const paints = vi.mocked(h.view.render).mock.calls.length;
    const phrases = vi.mocked(h.view.showPhrase).mock.calls.length;
    queued(5000);
    runtime.endReminder();
    expect(runtime.respond("click")).toBe(false);
    expect(h.view.render).toHaveBeenCalledTimes(paints);
    expect(h.view.showPhrase).toHaveBeenCalledTimes(phrases);
    expect(h.delays.size).toBe(0);
  });
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
  it("keeps reminder text with the cup until the native end, then returns the cup smoothly", () => {
    const h = harness(); const runtime = h.start();
    expect(runtime.respond("reminderDue", "先喝口水吧，咱们再继续！", "offerWater")).toBe(true);
    const paint = [...h.frames.values()][0]; paint(0);
    expect(h.view.render).toHaveBeenLastCalledWith({ asset: "waterOffer", row: 0, column: 0 });
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("先喝口水吧，咱们再继续！");
    expect([...h.delays.values()].some(delay => delay.ms === 3_000)).toBe(false);
    paint(10_000);
    expect(h.view.render).toHaveBeenLastCalledWith({ asset: "waterOffer", row: 1, column: 1 });
    expect(h.view.clearPhrase).not.toHaveBeenCalled();
    h.setNow(10_000); runtime.endReminder(); paint(10_220);
    expect(h.view.render).toHaveBeenLastCalledWith({ asset: "waterOffer", row: 1, column: 0 });
    expect(h.view.clearPhrase).not.toHaveBeenCalled();
    const returnDelay = [...h.delays.values()].find(delay => delay.ms === 880)!;
    returnDelay.callback(); paint(10_880);
    expect(h.view.clearPhrase).toHaveBeenCalledOnce();
    expect(h.view.render).toHaveBeenLastCalledWith(expect.not.objectContaining({ asset: "waterOffer" }));
    runtime.stop();
  });
  it("does not erase completion feedback when it follows the reminder end", () => {
    const h = harness(); const runtime = h.start();
    runtime.respond("reminderDue", "先喝口水吧，咱们再继续！", "offerWater");
    h.setNow(1_000); runtime.endReminder();
    const oldClear = [...h.delays.values()].find(delay => delay.ms === 880)!;
    runtime.respond("reminderCompleted");
    oldClear.callback();
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("好耶，先歇一小会儿！");
    expect(h.view.clearPhrase).not.toHaveBeenCalled();
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
  it("requests native passthrough only for blank space on a capable Windows host", async () => {
    const h = harness();
    const containsPointer = vi.fn().mockReturnValue(false);
    const setPointerRegionEnabled = vi.fn();
    const setPointerPassthrough = vi.fn().mockImplementation(async (ignore: boolean) => ignore);
    const character = { ...march7th, interaction: { pointerRegion: { kind: "visibleBounds" as const, paddingPx: 4 } } };
    const runtime = startPetRuntime({ character, view: { ...h.view, containsPointer, setPointerRegionEnabled }, host: { ...h.host, setPointerPassthrough }, gestures: h.gestures, scheduler: h.scheduler, reportError: h.reportError });
    await flush();
    expect(setPointerRegionEnabled).toHaveBeenCalledWith(true);
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(true, character.id);
    containsPointer.mockReturnValue(true);
    [...h.delays.values()].find(delay => delay.ms === 33)!.callback();
    await flush();
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(false, character.id);
    runtime.stop();
  });

  it("restores native input on stop and fences a late passthrough result", async () => {
    const h = harness(); const pending = deferred<boolean>();
    const containsPointer = vi.fn().mockReturnValue(false);
    const setPointerRegionEnabled = vi.fn();
    const setPointerPassthrough = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(false);
    const character = { ...march7th, interaction: { pointerRegion: { kind: "visibleBounds" as const, paddingPx: 4 } } };
    const runtime = startPetRuntime({ character, view: { ...h.view, containsPointer, setPointerRegionEnabled }, host: { ...h.host, setPointerPassthrough }, gestures: h.gestures, scheduler: h.scheduler, reportError: h.reportError });
    await flush();
    runtime.stop();
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(false, character.id, false);
    pending.resolve(true); await flush();
    expect(setPointerPassthrough).toHaveBeenCalledTimes(2);
    expect(h.delays.size).toBe(0);
    expect(setPointerRegionEnabled).toHaveBeenLastCalledWith(false);
  });

  it("keeps the previous interaction path when the host has no Windows pointer capability", async () => {
    const h = harness(); const enabled = vi.fn(); const containsPointer = vi.fn().mockReturnValue(false);
    const character = { ...march7th, interaction: { pointerRegion: { kind: "visibleBounds" as const, paddingPx: 4 } } };
    const runtime = startPetRuntime({ character, view: { ...h.view, containsPointer, setPointerRegionEnabled: enabled }, host: h.host, gestures: h.gestures, scheduler: h.scheduler, reportError: h.reportError });
    await flush();
    expect(enabled).not.toHaveBeenCalledWith(true);
    expect(containsPointer).not.toHaveBeenCalled();
    h.gesture("click");
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("咱在呢！");
    runtime.stop();
  });

  it("falls back to ordinary input when the native pointer policy fails", async () => {
    const h = harness();
    const enabled = vi.fn(); const containsPointer = vi.fn().mockReturnValue(false);
    const setPointerPassthrough = vi.fn().mockRejectedValueOnce(new Error("native unavailable")).mockResolvedValue(false);
    const character = { ...march7th, interaction: { pointerRegion: { kind: "visibleBounds" as const, paddingPx: 4 } } };
    const runtime = startPetRuntime({ character, view: { ...h.view, containsPointer, setPointerRegionEnabled: enabled }, host: { ...h.host, setPointerPassthrough }, gestures: h.gestures, scheduler: h.scheduler, reportError: h.reportError });
    await flush();
    expect(h.reportError).toHaveBeenCalledWith("pointer-region", expect.any(Error));
    expect(enabled).toHaveBeenLastCalledWith(false);
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(false, character.id, false);
    h.gesture("click");
    expect(h.view.showPhrase).toHaveBeenLastCalledWith("咱在呢！");
    runtime.stop();
  });
  it("rechecks a suppressed passthrough during held input and clears the request when the cursor returns to the body", async () => {
    const h = harness(); const containsPointer = vi.fn().mockReturnValue(false);
    const setPointerPassthrough = vi.fn().mockResolvedValue(false);
    const character = { ...march7th, interaction: { pointerRegion: { kind: "visibleBounds" as const, paddingPx: 4 } } };
    const runtime = startPetRuntime({ character, view: { ...h.view, containsPointer, setPointerRegionEnabled: vi.fn() }, host: { ...h.host, setPointerPassthrough }, gestures: h.gestures, scheduler: h.scheduler, reportError: h.reportError });
    await flush();
    [...h.delays.values()].find(delay => delay.ms === 33)!.callback(); await flush();
    expect(setPointerPassthrough).toHaveBeenCalledTimes(2);
    containsPointer.mockReturnValue(true);
    [...h.delays.values()].find(delay => delay.ms === 33)!.callback(); await flush();
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(false, character.id);
    runtime.stop();
  });

  it("restores native input for an unavailable cursor sample without permanently disabling the measured region", async () => {
    const h = harness(); const enabled = vi.fn(); const containsPointer = vi.fn().mockReturnValue(false);
    const setPointerPassthrough = vi.fn().mockImplementation(async (ignore: boolean) => ignore);
    h.host.sampleCursor.mockRejectedValueOnce(new Error("cursor unavailable"));
    const character = { ...march7th, interaction: { pointerRegion: { kind: "visibleBounds" as const, paddingPx: 4 } } };
    const runtime = startPetRuntime({ character, view: { ...h.view, containsPointer, setPointerRegionEnabled: enabled }, host: { ...h.host, setPointerPassthrough }, gestures: h.gestures, scheduler: h.scheduler, reportError: h.reportError });
    await flush();
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(false, character.id, false);
    expect(enabled).toHaveBeenLastCalledWith(true);
    [...h.delays.values()].find(delay => delay.ms === 33)!.callback(); await flush();
    expect(setPointerPassthrough).toHaveBeenLastCalledWith(true, character.id);
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
