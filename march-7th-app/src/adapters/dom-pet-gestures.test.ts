import { describe, expect, it, vi } from "vitest";
import type { Scheduler } from "../application/ports";
import { createDomPetGestures } from "./dom-pet-gestures";

type Listener = (event: any) => void;
function target() {
  const listeners = new Map<string, Set<Listener>>();
  return {
    node: {
      addEventListener(name: string, listener: Listener) {
        const bucket = listeners.get(name) ?? new Set();
        bucket.add(listener);
        listeners.set(name, bucket);
      },
      removeEventListener(name: string, listener: Listener) { listeners.get(name)?.delete(listener); },
      dispatch(name: string, event: any = {}) { for (const listener of listeners.get(name) ?? []) listener(event); },
    },
    listeners,
  };
}

function harness() {
  let now = 0;
  let id = 0;
  const delays = new Map<number, { at: number; callback: () => void }>();
  const scheduler: Scheduler = {
    now: () => now,
    requestFrame: () => 0,
    cancelFrame: () => undefined,
    setDelay: (callback, ms) => { delays.set(++id, { at: now + ms, callback }); return id; },
    cancelDelay: timerId => { delays.delete(timerId); },
  };
  const stage = target();
  const documentTarget = target();
  const windowTarget = target();
  const capture = new Set<number>();
  const stageNode = Object.assign(stage.node, {
    setPointerCapture: (pointerId: number) => capture.add(pointerId),
    hasPointerCapture: (pointerId: number) => capture.has(pointerId),
    releasePointerCapture: (pointerId: number) => capture.delete(pointerId),
  }) as unknown as HTMLElement;
  const input = createDomPetGestures(stageNode, documentTarget.node as unknown as Document, windowTarget.node as unknown as Window, scheduler);
  const handlers = { click: vi.fn(), doubleClick: vi.fn(), drag: vi.fn() };
  const dispose = input.subscribe(handlers);
  const pointer = (pointerId: number, x: number, y: number, extra: Record<string, unknown> = {}) => ({
    pointerId, clientX: x, clientY: y, button: 0, isPrimary: true, preventDefault: vi.fn(), ...extra,
  });
  const advance = (ms: number) => {
    now += ms;
    for (const [timerId, timer] of [...delays]) if (timer.at <= now) { delays.delete(timerId); timer.callback(); }
  };
  return { stage, documentTarget, windowTarget, capture, handlers, pointer, advance, delays, dispose };
}

describe("DOM pet gestures", () => {
  it("waits 300 ms before emitting a single click", () => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(1, 10, 10));
    h.documentTarget.node.dispatch("pointerup", h.pointer(1, 10, 10));
    h.advance(299);
    expect(h.handlers.click).not.toHaveBeenCalled();
    h.advance(1);
    expect(h.handlers.click).toHaveBeenCalledOnce();
    expect(h.handlers.doubleClick).not.toHaveBeenCalled();
  });

  it("turns two nearby releases into one double click with no single click", () => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(4, 20, 20));
    h.documentTarget.node.dispatch("pointerup", h.pointer(4, 20, 20));
    h.advance(120);
    h.stage.node.dispatch("pointerdown", h.pointer(7, 25, 23));
    h.documentTarget.node.dispatch("pointerup", h.pointer(7, 25, 23));
    expect(h.handlers.doubleClick).toHaveBeenCalledOnce();
    h.advance(300);
    expect(h.handlers.click).not.toHaveBeenCalled();
  });

  it("keeps releases more than six pixels apart as two single clicks", () => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(1, 0, 0));
    h.documentTarget.node.dispatch("pointerup", h.pointer(1, 0, 0));
    h.advance(100);
    h.stage.node.dispatch("pointerdown", h.pointer(2, 7, 0));
    h.documentTarget.node.dispatch("pointerup", h.pointer(2, 7, 0));
    expect(h.handlers.click).toHaveBeenCalledOnce();
    h.advance(300);
    expect(h.handlers.click).toHaveBeenCalledTimes(2);
    expect(h.handlers.doubleClick).not.toHaveBeenCalled();
  });

  it("starts drag once at six logical pixels, releases capture, and cancels pending click", () => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(1, 0, 0));
    h.documentTarget.node.dispatch("pointerup", h.pointer(1, 0, 0));
    h.advance(100);
    h.stage.node.dispatch("pointerdown", h.pointer(2, 20, 20));
    expect(h.capture.has(2)).toBe(true);
    h.documentTarget.node.dispatch("pointermove", h.pointer(2, 26, 20));
    h.documentTarget.node.dispatch("pointermove", h.pointer(2, 40, 20));
    expect(h.handlers.drag).toHaveBeenCalledOnce();
    expect(h.capture.has(2)).toBe(false);
    h.documentTarget.node.dispatch("pointerup", h.pointer(2, 40, 20));
    h.advance(300);
    expect(h.handlers.click).not.toHaveBeenCalled();
  });

  it("ignores non-primary, secondary and non-current pointers", () => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(1, 0, 0, { button: 2 }));
    h.stage.node.dispatch("pointerdown", h.pointer(2, 0, 0, { isPrimary: false }));
    h.stage.node.dispatch("pointerdown", h.pointer(3, 0, 0));
    h.documentTarget.node.dispatch("pointermove", h.pointer(99, 20, 0));
    h.documentTarget.node.dispatch("pointerup", h.pointer(99, 20, 0));
    expect(h.handlers.drag).not.toHaveBeenCalled();
    expect(h.delays.size).toBe(0);
  });

  it.each(["pointercancel", "lostpointercapture", "blur"])("cancels unfinished input on %s", eventName => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(3, 0, 0));
    if (eventName === "blur") h.windowTarget.node.dispatch(eventName);
    else h.documentTarget.node.dispatch(eventName, h.pointer(3, 0, 0));
    h.documentTarget.node.dispatch("pointerup", h.pointer(3, 0, 0));
    h.advance(300);
    expect(h.handlers.click).not.toHaveBeenCalled();
  });

  it("dispose cancels pending clicks and removes every listener", () => {
    const h = harness();
    h.stage.node.dispatch("pointerdown", h.pointer(3, 0, 0));
    h.documentTarget.node.dispatch("pointerup", h.pointer(3, 0, 0));
    h.dispose();
    h.advance(300);
    expect(h.handlers.click).not.toHaveBeenCalled();
    expect([...h.stage.listeners.values(), ...h.documentTarget.listeners.values(), ...h.windowTarget.listeners.values()].every(set => set.size === 0)).toBe(true);
  });
});
