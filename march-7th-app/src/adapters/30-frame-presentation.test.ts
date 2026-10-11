import { describe, expect, it, vi } from "vitest";
import { characterCatalog } from "../characters/catalog";
import { atlasBitmapDimensions, type CharacterDefinition } from "../domain/character";
import { createCharacterPresentationController } from "../application/character-presentation";
import { createDomPetView } from "./dom-pet-view";

const base = characterCatalog.characters[1];
const character: CharacterDefinition = {
  ...base,
  atlas: { ...base.atlas, columns: 30, rows: 11 },
  waterOffer: { kind: "mainAtlas", clip: { row: 5, frameCount: 30, frameIntervalMs: 37 } },
};
const flush = async () => { for (let count = 0; count < 6; count++) await Promise.resolve(); };

function element() {
  const style: Record<string, string> = {};
  const node = {
    style, dataset: {}, textContent: "", hidden: false,
    getBoundingClientRect: () => ({ left: 24, top: 26, width: 192, height: 208 }),
    setAttribute: () => {},
  } as unknown as HTMLElement;
  return { node, style };
}

describe("30-column main-atlas presentation", () => {
  it("loads only the physical main image before starting the runtime", async () => {
    const preloadAtlas = vi.fn().mockResolvedValue(atlasBitmapDimensions(character.atlas));
    const stop = vi.fn();
    const createRuntime = vi.fn(() => ({ stop, respond: vi.fn(), endReminder: vi.fn() }));
    const status = { setPresentationError: vi.fn() };
    const controller = createCharacterPresentationController({
      catalog: { defaultId: character.id, characters: [character] },
      initial: { characterId: character.id, revision: 1 },
      preloadAtlas, createRuntime, status, reportError: vi.fn(),
    });
    await flush();
    expect(preloadAtlas.mock.calls).toEqual([[character.atlas.src]]);
    expect(createRuntime).toHaveBeenCalledWith(character);
    expect(status.setPresentationError).toHaveBeenLastCalledWith(null);
    controller.destroy();
    expect(stop).toHaveBeenCalledOnce();
  });

  it("rejects an old8-column decoded image for the30-column definition", async () => {
    const preloadAtlas = vi.fn().mockResolvedValue({ width: 1536, height: 2288 });
    const createRuntime = vi.fn(() => ({ stop: vi.fn(), respond: vi.fn(), endReminder: vi.fn() }));
    const status = { setPresentationError: vi.fn() };
    const reportError = vi.fn();
    const controller = createCharacterPresentationController({
      catalog: { defaultId: character.id, characters: [character] },
      initial: { characterId: character.id, revision: 1 },
      preloadAtlas, createRuntime, status, reportError,
    });
    await flush();
    expect(createRuntime).not.toHaveBeenCalled();
    expect(status.setPresentationError).toHaveBeenLastCalledWith("无法显示雷电将军，请重试。");
    expect(reportError).toHaveBeenCalledOnce();
    controller.destroy();
  });

  it("draws cup columns0..29 and neutral on the same visible layer, then restores legacy geometry", () => {
    const sprite = element();
    const offer = element();
    const view = createDomPetView(sprite.node, offer.node, element().node, element().node, element().node);
    view.configure(character);
    expect(sprite.style.backgroundSize).toBe("5760px 2288px");
    for (let column = 0; column < 30; column++) {
      view.render({ row: 5, column });
      expect(sprite.style.backgroundPosition).toBe(`${-column * 192}px -1040px`);
      expect(sprite.style.opacity).toBe("1");
      expect(sprite.style.transition).toBe("none");
      expect(offer.style.opacity).toBe("0");
      expect(offer.style.display).toBe("none");
    }
    view.render({ row: 0, column: 0 });
    expect(sprite.style.backgroundPosition).toBe("0px 0px");
    expect(sprite.style.opacity).toBe("1");
    view.configure(characterCatalog.characters[0]);
    expect(sprite.style.backgroundSize).toBe("1536px 2288px");
    expect(sprite.style.transition).toBe("");
    expect(offer.style.display).toBe("");
    view.render({ asset: "waterOffer", row: 1, column: 1 });
    expect(offer.style.backgroundPosition).toBe("-192px -208px");
    expect(offer.style.opacity).toBe("1");
  });
});
