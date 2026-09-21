import { describe, expect, it, vi } from "vitest";
import { connectCharacterSelection, parseCharacterSnapshot } from "./tauri-characters";
const snapshot = (revision: number, id = "a") => ({ selectedCharacterId: id, revision, persistence: "saved" });
const flush = async () => { for (let i = 0; i < 10; i++) await Promise.resolve(); };
describe("native character selection boundary", () => {
  it("validates IPC fields and known identities", () => {
    expect(parseCharacterSnapshot(snapshot(1), ["a"])).toEqual({ characterId: "a", revision: 1, persistence: "saved" });
    for (const raw of [null, {}, snapshot(-1), snapshot(1.5), snapshot(1, "unknown"), { ...snapshot(1), persistence: "secret" }]) expect(() => parseCharacterSnapshot(raw, ["a"])).toThrow();
  });
  it("subscribes before read, ignores old revisions, allows retry and cleans up", async () => {
    let listener!: (value: unknown) => void; let resolve!: (value: unknown) => void;
    const get = vi.fn(() => new Promise<unknown>(r => { resolve = r; })); const select = vi.fn(); const stop = vi.fn();
    const dispose = connectCharacterSelection({ ids: ["a", "b"], select, reportError: vi.fn(), listen: async callback => { listener = callback; return stop; }, get });
    await flush(); expect(get).toHaveBeenCalledOnce(); listener(snapshot(2, "b")); resolve(snapshot(1)); await flush();
    listener(snapshot(0)); listener(snapshot(2, "b")); expect(select.mock.calls.map(call => call[0].characterId)).toEqual(["b", "b"]);
    dispose(); listener(snapshot(3)); expect(stop).toHaveBeenCalledOnce(); expect(select).toHaveBeenCalledTimes(2);
  });
  it("disposes a late listener without starting a read", async () => {
    let resolve!: (stop: () => void) => void; const stop = vi.fn(); const get = vi.fn();
    const dispose = connectCharacterSelection({ ids: ["a"], select: vi.fn(), reportError: vi.fn(), get, listen: () => new Promise(r => { resolve = r; }) });
    dispose(); resolve(stop); await flush(); expect(stop).toHaveBeenCalledOnce(); expect(get).not.toHaveBeenCalled();
  });
  it("ignores late reads and reports invalid events and startup failures", async () => {
    let listener!: (value: unknown) => void; let resolve!: (value: unknown) => void; const select = vi.fn(); const reportError = vi.fn();
    const dispose = connectCharacterSelection({ ids: ["a"], select, reportError, listen: async callback => { listener = callback; return () => {}; }, get: () => new Promise(r => { resolve = r; }) });
    await flush(); listener(snapshot(1, "unknown")); expect(reportError).toHaveBeenCalledOnce(); dispose(); resolve(snapshot(2)); await flush(); expect(select).not.toHaveBeenCalled();
    connectCharacterSelection({ ids: ["a"], select, reportError, listen: async () => { throw Error("listen"); }, get: vi.fn() }); await flush(); expect(reportError).toHaveBeenCalledTimes(2);
  });
});
