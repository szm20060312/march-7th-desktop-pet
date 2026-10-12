import { describe, expect, it, vi } from "vitest";
import { characterCatalog } from "../characters/catalog";
import { atlasBitmapDimensions, type CharacterDefinition } from "../domain/character";
import { createCharacterPresentationController, type SelectedCharacterSnapshot } from "./character-presentation";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const flush = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };

function harness(initial: SelectedCharacterSnapshot = { characterId: "march-7th", revision: 1 }) {
  const loads = new Map<string, ReturnType<typeof deferred<{ width: number; height: number }>>>();
  const preloadAtlas = vi.fn((src: string) => {
    const pending = deferred<{ width: number; height: number }>();
    loads.set(src, pending);
    return pending.promise;
  });
  const runtimes: Array<{ character: CharacterDefinition; stop: ReturnType<typeof vi.fn>; respond: ReturnType<typeof vi.fn>; endReminder: ReturnType<typeof vi.fn> }> = [];
  const createRuntime = vi.fn((character: CharacterDefinition) => {
    const runtime = { character, stop: vi.fn(), respond: vi.fn(), endReminder: vi.fn() };
    runtimes.push(runtime);
    return runtime;
  });
  const status = { setPresentationError: vi.fn() };
  const reportError = vi.fn();
  const controller = createCharacterPresentationController({ catalog: characterCatalog, initial, preloadAtlas, createRuntime, status, reportError });
  const finish = async (id: string, size?: { width: number; height: number }) => {
    const character = characterCatalog.characters.find(item => item.id === id)!;
    loads.get(character.atlas.src)!.resolve(size ?? atlasBitmapDimensions(character.atlas));
    if (character.waterOffer.kind !== "mainAtlas") loads.get(character.waterOffer.src)!.resolve({ width: character.waterOffer.sourceWidth, height: character.waterOffer.sourceHeight });
    await flush();
  };
  return { controller, preloadAtlas, createRuntime, runtimes, status, reportError, loads, finish };
}

describe("character presentation controller", () => {
  it("keeps only the latest live reminder for the loading opted-in character", async () => {
    const h = harness();
    await h.finish("march-7th");
    void h.controller.select({ characterId: "raiden-shogun", revision: 2 });
    expect(h.controller.remind(["water"], 21)).toBe(true);
    expect(h.controller.remind(["eyes"], 22)).toBe(true);
    expect(h.runtimes[0].respond).not.toHaveBeenCalled();
    await h.finish("raiden-shogun");
    expect(h.runtimes[1].respond.mock.calls).toEqual([["reminderDue", characterCatalog.characters[1].reminderPrompts.eyes[1], undefined]]);
  });

  it("does not defer legacy March 7th reminders while loading", async () => {
    const h = harness();
    expect(h.controller.remind(["water"], 21)).toBe(false);
    await h.finish("march-7th");
    expect(h.runtimes[0].respond).not.toHaveBeenCalled();
  });

  it("cancels a queued reminder when selection changes to another character", async () => {
    const h = harness({ characterId: "raiden-shogun", revision: 1 });
    expect(h.controller.remind(["water"], 21)).toBe(true);
    void h.controller.select({ characterId: "march-7th", revision: 2 });
    await h.finish("march-7th");
    await h.finish("raiden-shogun");
    expect(h.runtimes).toHaveLength(1);
    expect(h.runtimes[0].character.id).toBe("march-7th");
    expect(h.runtimes[0].respond).not.toHaveBeenCalled();
  });

  it("does not resurrect an ended reminder after a failed decode and retry", async () => {
    const h = harness({ characterId: "raiden-shogun", revision: 1 });
    expect(h.controller.remind(["water"], 21)).toBe(true);
    await h.finish("raiden-shogun", { width: 1, height: 1 });
    h.controller.endReminder();
    void h.controller.select({ characterId: "raiden-shogun", revision: 1 });
    await h.finish("raiden-shogun");
    expect(h.runtimes[0].respond).not.toHaveBeenCalled();
  });

  it("drops a pending reminder when the controller is destroyed", async () => {
    const h = harness({ characterId: "raiden-shogun", revision: 1 });
    expect(h.controller.remind(["water"], 21)).toBe(true);
    h.controller.destroy();
    await h.finish("raiden-shogun");
    expect(h.createRuntime).not.toHaveBeenCalled();
  });

  it("preloads only the main image for a main-atlas cup and still validates its decoded size", async () => {
    const source = characterCatalog.characters[1];
    const character: CharacterDefinition = { ...source, waterOffer: { kind: "mainAtlas", clip: { row: 5, frameCount: 8, frameIntervalMs: 140 } } };
    const preloadAtlas = vi.fn().mockResolvedValue(atlasBitmapDimensions(source.atlas));
    const createRuntime = vi.fn(() => ({ stop: vi.fn(), respond: vi.fn(), endReminder: vi.fn() }));
    const status = { setPresentationError: vi.fn() };
    const controller = createCharacterPresentationController({ catalog: { defaultId: character.id, characters: [character] },
      initial: { characterId: character.id, revision: 1 }, preloadAtlas, createRuntime, status, reportError: vi.fn() });
    await flush();
    expect(preloadAtlas.mock.calls).toEqual([[source.atlas.src]]);
    expect(createRuntime).toHaveBeenCalledWith(character);
    controller.destroy();
    preloadAtlas.mockResolvedValue({ width: 1, height: 1 });
    createRuntime.mockClear();
    const failed = createCharacterPresentationController({ catalog: { defaultId: character.id, characters: [character] },
      initial: { characterId: character.id, revision: 1 }, preloadAtlas, createRuntime, status, reportError: vi.fn() });
    await flush();
    expect(createRuntime).not.toHaveBeenCalled();
    expect(status.setPresentationError).toHaveBeenLastCalledWith("无法显示雷电将军，请重试。");
    failed.destroy();
  });
  it("validates decoded atlas dimensions before starting the initial runtime", async () => {
    const h = harness();
    expect(h.createRuntime).not.toHaveBeenCalled();
    await h.finish("march-7th");
    expect(h.runtimes[0].character.id).toBe("march-7th");
    expect(h.status.setPresentationError).toHaveBeenLastCalledWith(null);
  });
  it("preloads the matching water action before selecting a character and forwards its animation", async () => {
    const h = harness();
    expect(h.preloadAtlas.mock.calls.map(([src]) => src)).toContain("/assets/march-7th/water-offer.png");
    await h.finish("march-7th");
    h.controller.remind(["water"], 1);
    expect(h.runtimes[0].respond).toHaveBeenLastCalledWith("reminderDue", "先喝口水吧，咱们再继续！", "offerWater");
    h.controller.endReminder();
    expect(h.runtimes[0].endReminder).toHaveBeenCalledOnce();
  });

  it("keeps the old runtime until preload succeeds, then stops it before starting the new one", async () => {
    const h = harness();
    await h.finish("march-7th");
    const old = h.runtimes[0];
    const order: string[] = [];
    old.stop.mockImplementation(() => { order.push("stop-old"); });
    h.createRuntime.mockImplementationOnce(character => {
      order.push("start-new");
      const runtime = { character, stop: vi.fn(), respond: vi.fn(), endReminder: vi.fn() };
      h.runtimes.push(runtime);
      return runtime;
    });
    void h.controller.select({ characterId: "raiden-shogun", revision: 2 });
    expect(old.stop).not.toHaveBeenCalled();
    await h.finish("raiden-shogun");
    expect(order).toEqual(["stop-old", "start-new"]);
  });

  it("does not let late loads or stale snapshots replace a newer selection", async () => {
    const h = harness();
    await h.finish("march-7th");
    void h.controller.select({ characterId: "raiden-shogun", revision: 2 });
    void h.controller.select({ characterId: "march-7th", revision: 3 });
    await h.finish("march-7th");
    await h.finish("raiden-shogun");
    await h.controller.select({ characterId: "raiden-shogun", revision: 2 });
    expect(h.runtimes[h.runtimes.length - 1].character.id).toBe("march-7th");
  });

  it("does not rebuild the active id and retries a failed switch without claiming success", async () => {
    const h = harness();
    await h.finish("march-7th");
    await h.controller.select({ characterId: "march-7th", revision: 2 });
    expect(h.createRuntime).toHaveBeenCalledTimes(1);

    const failed = h.controller.select({ characterId: "raiden-shogun", revision: 3 });
    h.loads.get("/assets/raiden-shogun/spritesheet.webp")!.resolve({ width: 1, height: 1 });
    const offer = characterCatalog.characters.find(item => item.id === "raiden-shogun")!.waterOffer;
    if (offer.kind !== "mainAtlas") h.loads.get(offer.src)!.resolve({ width: offer.sourceWidth, height: offer.sourceHeight });
    await failed;
    expect(h.runtimes[h.runtimes.length - 1].character.id).toBe("march-7th");
    expect(h.status.setPresentationError).toHaveBeenLastCalledWith("无法显示雷电将军，请重试。");
    expect(h.reportError).toHaveBeenLastCalledWith(expect.objectContaining({ id: "raiden-shogun" }), expect.objectContaining({ message: expect.stringMatching(/尺寸/) }));

    void h.controller.select({ characterId: "raiden-shogun", revision: 3 });
    await h.finish("raiden-shogun");
    expect(h.runtimes[h.runtimes.length - 1].character.id).toBe("raiden-shogun");
  });

  it("reports initial failure and ignores all late work after destroy", async () => {
    const h = harness();
    h.loads.get("/assets/march-7th/spritesheet.webp")!.reject(new Error("decode failed"));
    await flush();
    expect(h.status.setPresentationError).toHaveBeenLastCalledWith("无法显示三月七，请重试。");
    expect(h.reportError).toHaveBeenLastCalledWith(expect.objectContaining({ id: "march-7th" }), expect.objectContaining({ message: "decode failed" }));
    const beforeDestroyWrites = h.status.setPresentationError.mock.calls.length;
    void h.controller.select({ characterId: "raiden-shogun", revision: 2 });
    h.controller.destroy();
    await h.finish("raiden-shogun");
    expect(h.createRuntime).not.toHaveBeenCalled();
    expect(h.status.setPresentationError).toHaveBeenCalledTimes(beforeDestroyWrites);
  });
  it("restores the previous runtime if starting the decoded replacement throws", async () => {
    const h = harness();
    await h.finish("march-7th");
    const old = h.runtimes[0];
    h.createRuntime.mockImplementationOnce(() => { throw new Error("runtime failed"); });
    void h.controller.select({ characterId: "raiden-shogun", revision: 2 });
    await h.finish("raiden-shogun");
    expect(old.stop).toHaveBeenCalledOnce();
    expect(h.runtimes[h.runtimes.length - 1].character.id).toBe("march-7th");
    expect(h.status.setPresentationError).toHaveBeenLastCalledWith("无法显示雷电将军，请重试。");
    const callsBeforeSameId = h.createRuntime.mock.calls.length;
    await h.controller.select({ characterId: "march-7th", revision: 3 });
    expect(h.createRuntime).toHaveBeenCalledTimes(callsBeforeSameId);
  });
});
