import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({ invoke: vi.fn(), startDragging: vi.fn(), listen: vi.fn(), getSelection: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (command: string) => command === "get_selected_character" ? native.getSelection() : native.invoke(command) }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging: native.startDragging }),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));

// A minimal host double: assertions observe rendered frames and tracking state,
// not private animation variables. The same tests run before and after extraction.
let now: number;
let frame: FrameRequestCallback;
let timers: Map<number, { callback: () => void; ms: number }>;
let timerId: number;
let stageTarget: ReturnType<typeof eventTarget>;
let documentTarget: ReturnType<typeof eventTarget>;
let windowTarget: ReturnType<typeof eventTarget>;
let sprite: { style: Record<string, unknown>; dataset: Record<string, string> };
let body: { dataset: Record<string, string> };
let message: { textContent: string; hidden: boolean; dataset: Record<string, string> };
const sample = (x = 200, y = 100, windowX = 0, windowY = 0) => ({ x, y, windowX, windowY });
const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
type Listener = (event: any) => void;
function eventTarget() {
  const listeners = new Map<string, Set<Listener>>();
  return {
    listeners,
    addEventListener(name: string, listener: Listener) {
      const bucket = listeners.get(name) ?? new Set();
      bucket.add(listener);
      listeners.set(name, bucket);
    },
    removeEventListener(name: string, listener: Listener) { listeners.get(name)?.delete(listener); },
    dispatch(name: string, event: any = {}) { for (const listener of listeners.get(name) ?? []) listener(event); },
  };
}

beforeEach(() => {
  vi.resetModules();
  native.listen.mockReset().mockResolvedValue(vi.fn());
  native.getSelection.mockReset().mockResolvedValue({ selectedCharacterId: "march-7th", revision: 0, persistence: "default" });
  native.invoke.mockReset().mockResolvedValue(sample());
  native.startDragging.mockReset().mockResolvedValue(undefined);
  now = 0;
  timers = new Map();
  timerId = 0;
  sprite = { style: {}, dataset: {} };
  body = { dataset: {} };
  const element = {
    ...sprite,
    getBoundingClientRect: () => ({ left: 4, top: -4, width: 192, height: 208 }),
    setAttribute: vi.fn(),
  };
  message = { textContent: "", hidden: true, dataset: {} };
  stageTarget = eventTarget();
  const capture = new Set<number>();
  const stage = Object.assign(stageTarget, {
    setAttribute: vi.fn(),
    setPointerCapture: (id: number) => capture.add(id),
    hasPointerCapture: (id: number) => capture.has(id),
    releasePointerCapture: (id: number) => capture.delete(id),
  });
  documentTarget = eventTarget();
  const documentDouble = Object.assign(documentTarget, {
    body,
    querySelector: (selector: string) => selector === "#pet-sprite" ? element : selector === "#pet-message" ? message : stage,
  });
  vi.stubGlobal("document", documentDouble);
  vi.stubGlobal("performance", { now: () => now });
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { frame = callback; return 1; });
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  windowTarget = eventTarget();
  vi.stubGlobal("window", Object.assign(windowTarget, {
    setTimeout: (callback: () => void, ms: number) => { timers.set(++timerId, { callback, ms }); return timerId; },
    clearTimeout: (id: number) => { timers.delete(id); },
  }));
  vi.stubGlobal("Image", class {
    src = "";
    naturalWidth = 1536;
    naturalHeight = 2288;
    decode = vi.fn().mockResolvedValue(undefined);
  });
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

async function boot() { await import("./main"); await flush(); }
async function poll(value: ReturnType<typeof sample>, at: number) {
  now = at;
  native.invoke.mockResolvedValueOnce(value);
  const entry = [...timers].find(([, timer]) => timer.ms === 33)!;
  timers.delete(entry[0]);
  entry[1].callback();
  await flush();
}
function paint(at: number) { now = at; frame(at); return sprite.dataset.frame; }

describe("desktop pet behavior at the entry point", () => {
  it("subscribes once to native reminder responses and routes them to the current character", async () => {
    await boot();
    const listeners = native.listen.mock.calls.filter(call => call[0] === "reminder-response");
    expect(listeners).toHaveLength(1);
    const respond = listeners[0][1];
    respond({ payload: { revision: 1, type: "complete", id: "water" } });
    expect(["好啦，继续慢慢来。", "照顾好自己呀。"]).toContain(message.textContent);
    const timerCount = timers.size; respond({ payload: { revision: 1, type: "complete", id: "water" } }); expect(timers.size).toBe(timerCount);
    native.listen.mock.calls.find(call => call[0] === "selected-character-changed")![1]({ payload: { selectedCharacterId: "raiden-shogun", revision: 2, persistence: "saved" } }); await flush();
    respond({ payload: { revision: 3, type: "snoozeAll" } }); expect(["不急，依你的节奏。", "待你方便，再作提醒。"]).toContain(message.textContent);
    windowTarget.dispatch("pagehide"); const text = message.textContent; respond({ payload: { revision: 4, type: "complete", id: "eyes" } }); expect(message.textContent).toBe(text);
  });
  it("does not queue reminder responses received before a character is ready", async () => {
    let resolve!: (value: unknown) => void; native.getSelection.mockReturnValue(new Promise(r => { resolve = r; }));
    await boot(); native.listen.mock.calls.find(call => call[0] === "reminder-response")![1]({ payload: { revision: 1, type: "snoozeAll" } });
    resolve({ selectedCharacterId: "march-7th", revision: 1, persistence: "saved" }); await flush(); expect(message.hidden).toBe(true);
  });
  it("keeps a restart instruction visible when the initial listener cannot be established", async () => {
    native.listen.mockRejectedValue(Error("listener unavailable"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    await boot();
    expect(message.textContent).toBe("角色连接失败，请重启。");
    expect(message.textContent).not.toContain("托盘");
    expect(message.hidden).toBe(false);
    expect(native.getSelection).not.toHaveBeenCalled();
    expect(sprite.style.backgroundImage).toBeUndefined();
    for (const timer of [...timers.values()]) timer.callback();
    expect(message.textContent).toBe("角色连接失败，请重启。");
    expect(message.hidden).toBe(false);
  });
  it("can recover a failed initial read through an already established native listener", async () => {
    native.getSelection.mockRejectedValue(Error("read unavailable"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    await boot(); expect(message.textContent).toContain("托盘重试");
    native.listen.mock.calls[0][1]({ payload: { selectedCharacterId: "raiden-shogun", revision: 3, persistence: "saved" } });
    await flush();
    expect(sprite.style.backgroundImage).toContain("raiden-shogun");
    expect(message.hidden).toBe(true);
  });
  it.each(["listen", "get"])("does not show a late %s failure after page disposal", async phase => {
    let reject!: (error: Error) => void;
    const pending = new Promise((_, r) => { reject = r; });
    if (phase === "listen") native.listen.mockReturnValue(pending);
    else native.getSelection.mockReturnValue(pending);
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    await boot(); windowTarget.dispatch("pagehide"); reject(Error("late failure")); await flush();
    expect(message.hidden).toBe(true); expect(message.textContent).toBe(""); expect(log).not.toHaveBeenCalled();
  });
  it("starts from the real native choice without first configuring the default", async () => {
    native.getSelection.mockResolvedValue({ selectedCharacterId: "raiden-shogun", revision: 7, persistence: "saved" });
    await boot();
    expect(sprite.style.backgroundImage).toContain("raiden-shogun");
  });
  it("keeps the old runtime on atlas failure, clears the short error and retries the same selection", async () => {
    await boot();
    let fail = true;
    vi.stubGlobal("Image", class {
      src = ""; naturalWidth = 1536; naturalHeight = 2288;
      decode = async () => { if (fail) throw Error("simulated atlas failure"); };
    });
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    const event = native.listen.mock.calls[0][1];
    const payload = { selectedCharacterId: "raiden-shogun", revision: 1, persistence: "saved" };
    event({ payload }); await flush();
    expect(sprite.style.backgroundImage).toContain("march-7th");
    expect(message.textContent).toContain("无法显示雷电将军");
    const timeout = [...timers].find(([, timer]) => timer.ms === 4000)!;
    timeout[1].callback(); expect(message.hidden).toBe(true);
    fail = false; event({ payload }); await flush();
    expect(sprite.style.backgroundImage).toContain("raiden-shogun");
    expect(log).toHaveBeenCalledOnce(); log.mockRestore();
  });
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
    const entry = [...timers].find(([, timer]) => timer.ms === 33)!;
    timers.delete(entry[0]);
    entry[1].callback();
    await flush();
    expect(body.dataset.cursorTracking).toBe("unavailable");
    expect(paint(10)).toBe("0:0");
    await poll(sample(), 33);
    expect(paint(33)).toBe("9:4");
  });
  it("starts native drag only for the primary button", async () => {
    await boot();
    const pointer = (button: number, x: number) => ({ button, pointerId: 1, isPrimary: true, clientX: x, clientY: 0, preventDefault: vi.fn() });
    stageTarget.dispatch("pointerdown", pointer(2, 0));
    expect(native.startDragging).not.toHaveBeenCalled();
    stageTarget.dispatch("pointerdown", pointer(0, 0));
    expect(native.startDragging).not.toHaveBeenCalled();
    documentTarget.dispatch("pointermove", pointer(0, 6));
    expect(native.startDragging).toHaveBeenCalledOnce();
    expect(paint(0)).toBe("1:0");
  });
});
