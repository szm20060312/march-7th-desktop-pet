import { describe, expect, it, vi } from "vitest";
import rawCatalog from "../characters/catalog.json";
import { characterCatalog, parseCharacterCatalog } from "../characters/catalog";
import { createCharacterPresentationController } from "../application/character-presentation";
import { createDomPetView } from "./dom-pet-view";

const flush = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };
const source = characterCatalog.characters.find(c => c.id === "raiden-shogun")!;
const dense = { ...source, atlas: { ...source.atlas, columns: 30, pixelDensity: 2 as const },
  waterOffer: { kind: "mainAtlas" as const, clip: { row: 5, frameCount: 1, frameIntervalMs: 20 } } };

describe("high density atlas preserves logical geometry", () => {
  it("keeps declared density while retaining legacy character metadata", () => {
    const data = JSON.parse(JSON.stringify(rawCatalog));
    data.characters[1].atlas.pixelDensity = 2;
    const parsed = parseCharacterCatalog(data);
    expect(parsed.characters[1].atlas).toMatchObject({ pixelDensity: 2, cellWidth: 192, cellHeight: 208 });
    expect(parsed.characters[0].atlas).toEqual(characterCatalog.characters[0].atlas);
  });
  it.each([0, 3, -1, 1.5, "2", null])("rejects unsupported pixel density %s", density => {
    const data = JSON.parse(JSON.stringify(rawCatalog));
    data.characters[1].atlas.pixelDensity = density;
    expect(() => parseCharacterCatalog(data)).toThrow(/pixelDensity/);
  });
  it.each([true, false])("validates physical bitmap before starting runtime, full bitmap=%s", async full => {
    const createRuntime = vi.fn(() => ({ stop: vi.fn(), respond: () => true, endReminder: vi.fn() }));
    const controller = createCharacterPresentationController({
      catalog: { defaultId: dense.id, characters: [dense] }, initial: { characterId: dense.id, revision: 1 },
      preloadAtlas: async () => ({ width: 5760 * (full ? 2 : 1), height: 2288 * (full ? 2 : 1) }),
      createRuntime, status: { setPresentationError: vi.fn() }, reportError: vi.fn(),
    });
    await flush();
    expect(createRuntime).toHaveBeenCalledTimes(full ? 1 : 0);
    controller.destroy();
  });
  it("keeps displayed frame and offsets in logical pixels for dense image", () => {
    const node = () => ({ style: {}, dataset: {}, setAttribute: vi.fn(),
      getBoundingClientRect: () => ({ left: 24, top: 26, width: 192, height: 208 }) }) as unknown as HTMLElement;
    const sprite = node();
    const view = createDomPetView(sprite, node(), node(), node(), node());
    view.configure(dense); view.render({ row: 9, column: 15 });
    expect(sprite.style.width).toBe("192px"); expect(sprite.style.height).toBe("208px");
    expect(sprite.style.backgroundSize).toBe("5760px 2288px");
    expect(sprite.style.backgroundPosition).toBe("-2880px -1872px");
    expect(view.center()).toEqual({ x: 120, y: 130 });
  });
});
