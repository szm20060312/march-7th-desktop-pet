import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({ invoke: vi.fn(), startDragging: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging: native.startDragging }),
}));

// A minimal host double: assertions observe rendered frames and tracking state,
// not private animation variables. The same tests run before and after extraction.
let now: number;
let frame: FrameRequestCallback;
let timers: Array<() => void>;
let pointerDown: (event: { button: number }) => void;
let sprite: { style: Record<string, unknown>; dataset: Record<string, string> };
let body: { dataset: Record<string, string> };
const sample = (x = 200, y = 100, windowX = 0, windowY = 0) => ({ x, y, windowX, windowY });
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };

beforeEach(() => {
  vi.resetModules();
  native.invoke.mockReset().mockResolvedValue(sample());
  native.startDragging.mockReset().mockResolvedValue(undefined);
  now = 0;
  timers = [];
  sprite = { style: {}, dataset: {} };
  body = { dataset: {} };
  const element = {
    ...sprite,
    getBoundingClientRect: () => ({ left: 4, top: -4, width: 192, height: 208 }),
    setAttribute: vi.fn(),
  };
  const stage = {
    setAttribute: vi.fn(),
    addEventListener: (_: string, handler: typeof pointerDown) => { pointerDown = handler; },
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("document", { body, querySelector: (selector: string) => selector === "#pet-sprite" ? element : stage });
  vi.stubGlobal("performance", { now: () => now });
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { frame = callback; return 1; });
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  vi.stubGlobal("window", {
    setTimeout: (callback: () => void) => { timers.push(callback); return timers.length; },
    clearTimeout: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
});
afterEach(() => vi.unstubAllGlobals());

async function boot() { await import("./main"); await flush(); }
async function poll(value: ReturnType<typeof sample>, at: number) {
  now = at;
  native.invoke.mockResolvedValueOnce(value);
  timers.shift()!();
  await flush();
}
function paint(at: number) { now = at; frame(at); return sprite.dataset.frame; }

describe("desktop pet behavior at the entry point", () => {
  it("shows a static look frame outside the deadzone", async () => {
    await boot();
    expect(paint(0)).toBe("9:4");
    expect(paint(1000)).toBe("9:4");
  });
  it("cycles six idle frames at 280 ms inside the deadzone", async () => {
    native.invoke.mockResolvedValue(sample(100, 100));
    await boot();
    expect(paint(279)).toBe("0:0");
    for (let i = 1; i <= 6; i++) expect(paint(i * 280)).toBe(`0:${i % 6}`);
  });
  it("suppresses gaze during movement and restores it after 160 ms", async () => {
    await boot();
    await poll(sample(200, 100, 4), 33);
    expect(paint(33)).toBe("1:0");
    expect(paint(123)).toBe("1:1");
    await poll(sample(200, 100, 4), 192);
    expect(paint(192)).toBe("1:1");
    await poll(sample(200, 100, 4), 193);
    expect(paint(193)).toBe("9:4");
  });
  it("resets the movement clip on reversal and retains direction vertically", async () => {
    await boot();
    await poll(sample(200, 100, -4), 33);
    expect(paint(33)).toBe("2:0");
    await poll(sample(200, 100, -4, 5), 66);
    expect(paint(66)).toBe("2:0");
    await poll(sample(200, 100, 2, 5), 99);
    expect(paint(99)).toBe("1:0");
  });
  it("ignores movement noise at or below half a pixel", async () => {
    await boot();
    await poll(sample(200, 100, 0.5, 0.5), 33);
    expect(paint(33)).toBe("9:4");
  });
  it("continues polling and falls back to idle after a sample failure", async () => {
    await boot();
    native.invoke.mockRejectedValueOnce(new Error("unavailable"));
    timers.shift()!();
    await flush();
    expect(body.dataset.cursorTracking).toBe("unavailable");
    expect(paint(10)).toBe("0:0");
    await poll(sample(), 33);
    expect(paint(33)).toBe("9:4");
  });
  it("starts native drag only for the primary button", async () => {
    await boot();
    pointerDown({ button: 2 });
    expect(native.startDragging).not.toHaveBeenCalled();
    pointerDown({ button: 0 });
    expect(native.startDragging).toHaveBeenCalledOnce();
    expect(paint(0)).toBe("1:0");
  });
});
