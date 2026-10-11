import { describe, expect, it } from 'vitest';
import { PetModel } from './pet-model';
import { march7th } from '../characters/march-7th';
import { parseCharacterCatalog } from '../characters/catalog';
import data from '../characters/catalog.json';

const candidate = { ...march7th, waterOffer: { ...march7th.waterOffer, columns: 4, rows: 3, frameCount: 12, frameIntervalMs: 80 } };
const center = { x: 0, y: 0 };

describe('configurable water action', () => {
  it('accepts a twelve-frame grid without changing the legacy character', () => {
    const catalog = structuredClone(data);
    Object.assign(catalog.characters[0].waterOffer, candidate.waterOffer);
    const result = parseCharacterCatalog(catalog);
    expect(result.characters[0].waterOffer.frameCount).toBe(12);
    expect(result.characters[1].waterOffer).toEqual(data.characters[1].waterOffer);
    catalog.characters[0].waterOffer.frameCount = 13;
    expect(() => parseCharacterCatalog(catalog)).toThrow(/grid|frameCount/);
  });
  it('holds frame eleven and returns through the configured grid to frame zero', () => {
    const model = new PetModel(candidate, 0);
    model.respond('offerWater', 0);
    expect(model.frameAt(880, center)).toEqual({ asset: 'waterOffer', row: 2, column: 3 });
    expect(model.frameAt(5000, center)).toEqual({ asset: 'waterOffer', row: 2, column: 3 });
    expect(model.endOfferWater(5000)).toBe(960);
    expect(model.frameAt(5080, center)).toEqual({ asset: 'waterOffer', row: 2, column: 2 });
    expect(model.frameAt(5880, center)).toEqual({ asset: 'waterOffer', row: 0, column: 0 });
    expect(model.frameAt(5960, center).asset).toBeUndefined();
  });
  it('an early end reverses from the actual pose without jumping forward', () => {
    const model = new PetModel(candidate, 0);
    model.respond('offerWater', 0);
    expect(model.frameAt(240, center)).toEqual({ asset: 'waterOffer', row: 0, column: 3 });
    expect(model.endOfferWater(240)).toBe(320);
    expect(model.frameAt(240, center)).toEqual({ asset: 'waterOffer', row: 0, column: 3 });
    expect(model.frameAt(320, center)).toEqual({ asset: 'waterOffer', row: 0, column: 2 });
    expect(model.endOfferWater(330)).toBe(0);
    expect(model.frameAt(480, center)).toEqual({ asset: 'waterOffer', row: 0, column: 0 });
    expect(model.frameAt(560, center).asset).toBeUndefined();
  });
  it('movement aborts the return and a new reminder starts a fresh action', () => {
    const model = new PetModel(candidate, 0);
    model.respond('offerWater', 0);
    model.endOfferWater(240);
    model.beginDrag(260);
    expect(model.frameAt(260, center).asset).toBeUndefined();
    expect(model.respond('offerWater', 270)).toBe(false);
    expect(model.respond('offerWater', 500)).toBe(true);
    expect(model.frameAt(500, center)).toEqual({ asset: 'waterOffer', row: 0, column: 0 });
  });
  it('restores the live gaze after an early return', () => {
    const model = new PetModel(candidate, 0);
    model.acceptSample({ x: 100, y: 0, windowX: 0, windowY: 0 }, 0);
    model.respond('offerWater', 0);
    model.endOfferWater(40);
    expect(model.frameAt(119, center)).toEqual({ asset: 'waterOffer', row: 0, column: 0 });
    expect(model.frameAt(120, center)).toEqual({ row: 9, column: 4 });
  });
});
