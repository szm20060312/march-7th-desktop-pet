import { describe, expect, it, vi } from "vitest";
import { mountDevConsole } from "./dom-dev-console";

const mockInvoke = vi.hoisted(() => vi.fn());
const mockListen = vi.hoisted(() => vi.fn().mockResolvedValue(() => {}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));

function makeNode() {
  const listeners = new Map<string, ((event: unknown) => void)[]>();
  return {
    hidden: true,
    textContent: "",
    scrollTop: 0,
    scrollHeight: 100,
    addEventListener(type: string, fn: (event: unknown) => void) {
      listeners.set(type, [...(listeners.get(type) ?? []), fn]);
    },
    removeEventListener(type: string, fn: (event: unknown) => void) {
      listeners.set(type, (listeners.get(type) ?? []).filter(item => item !== fn));
    },
    dispatch(type: string, event: unknown = {}) {
      for (const fn of listeners.get(type) ?? []) fn(event);
    },
  };
}

describe("dom dev console unlock", () => {
  it("unlocks after 8 rapid taps on build-version and resets after window timeout", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_debug_logs") return [{ timestamp: "20:00:00.000", category: "menu", message: "ok" }];
      if (cmd === "get_debug_state") return { ok: true };
      return undefined;
    });
    const nodes = new Map([
      ["build-version", makeNode()],
      ["build-status", makeNode()],
      ["dev-console", makeNode()],
      ["dev-log-output", makeNode()],
      ["dev-state-output", makeNode()],
      ["dev-refresh", makeNode()],
      ["dev-clear-logs", makeNode()],
    ]);
    const doc = { getElementById: (id: string) => nodes.get(id) ?? null } as unknown as Document;
    let now = 1000;
    const stop = mountDevConsole(doc, () => now);
    const version = nodes.get("build-version")!;
    const panel = nodes.get("dev-console")!;

    for (let i = 0; i < 6; i++) {
      now += 100;
      version.dispatch("click");
    }
    expect(panel.hidden).toBe(true);
    expect(nodes.get("build-status")!.textContent).toContain("2 次");

    now += 2000; // exceed tap window, resets count to 1
    version.dispatch("click");
    expect(panel.hidden).toBe(true);

    for (let i = 0; i < 7; i++) {
      now += 100;
      version.dispatch("click");
    }
    expect(panel.hidden).toBe(false);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(nodes.get("dev-log-output")!.textContent).toContain("[menu] ok");
    stop();
  });
});
