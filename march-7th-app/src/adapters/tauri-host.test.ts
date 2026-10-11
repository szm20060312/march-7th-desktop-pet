import { afterEach, describe, expect, it, vi } from "vitest";
import { createTauriHost, parseCursorSample } from "./tauri-host";

const native = vi.hoisted(() => ({ invoke: vi.fn(), startDragging: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ startDragging: native.startDragging }) }));
afterEach(() => { vi.unstubAllGlobals(); native.invoke.mockReset(); });

describe("cursor IPC boundary", () => {
  it("accepts negative and fractional coordinates for multi-display/DPI use", () => {
    const sample = { x: -30.5, y: 8.25, windowX: -1920, windowY: -1080 };
    expect(parseCursorSample(sample)).toEqual(sample);
  });
  it.each([null, {}, { x: 0, y: 0, windowX: NaN, windowY: 0 },
    { x: Infinity, y: 0, windowX: 0, windowY: 0 }, { x: "1", y: 0, windowX: 0, windowY: 0 },
  ])("rejects malformed samples instead of poisoning animation state", value => {
    expect(() => parseCursorSample(value)).toThrow();
  });
});

describe("Windows selective pointer transport", () => {
  it("is absent on macOS and serializes Windows changes through the registered main-window command", async () => {
    vi.stubGlobal("navigator", { platform: "MacIntel" });
    expect(createTauriHost().setPointerPassthrough).toBeUndefined();
    vi.stubGlobal("navigator", { platform: "Win32" });
    let resolve!: (value: boolean) => void;
    native.invoke.mockReturnValueOnce(new Promise<boolean>(yes => { resolve = yes; })).mockResolvedValue(false);
    const host = createTauriHost();
    const enable = host.setPointerPassthrough!(true, "capable");
    const stop = host.setPointerPassthrough!(false, "capable", false);
    for (let i = 0; i < 4; i++) await Promise.resolve();
    expect(native.invoke).toHaveBeenCalledTimes(1);
    expect(native.invoke).toHaveBeenLastCalledWith("set_pet_pointer_passthrough", { ignore: true, enabled: true, characterId: "capable", sequence: expect.any(Number) });
    resolve(true); await enable; await stop;
    const first = native.invoke.mock.calls[0][1]; const second = native.invoke.mock.calls[1][1];
    expect(second).toMatchObject({ ignore: false, enabled: false, characterId: "capable" });
    expect(Number.isSafeInteger(second.sequence)).toBe(true);
    expect(second.sequence).toBeGreaterThan(first.sequence);
  });

  it("rejects an invalid native result and still allows restoring input after a failed request", async () => {
    vi.stubGlobal("navigator", { platform: "Win32" });
    native.invoke.mockResolvedValueOnce({ ignored: true }).mockResolvedValue(false);
    const host = createTauriHost();
    await expect(host.setPointerPassthrough!(true, "capable")).rejects.toThrow(/Invalid/);
    await expect(host.setPointerPassthrough!(false, "capable", false)).resolves.toBe(false);
  });
});
