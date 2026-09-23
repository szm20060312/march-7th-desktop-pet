import { describe, expect, it } from "vitest";
import { march7th } from "../characters/march-7th";
import { PetModel } from "./pet-model";
import { directionFrame } from "./animation";

const center = { x: 0, y: 0 };
const sample = (windowX: number, windowY = 0) => ({ x: 100, y: 0, windowX, windowY });

describe("PetModel", () => {
  it("extends the quiet gaze period on each real window movement", () => {
    const pet = new PetModel(march7th, 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 100);
    pet.acceptSample(sample(20), 200);
    expect(pet.isMoving(359)).toBe(true);
    expect(pet.isMoving(360)).toBe(false);
    pet.acceptSample(sample(20), 360);
    expect(pet.frameAt(360, center)).toEqual({ row: 9, column: 4 });
  });
  it("cycles the movement clip without showing gaze", () => {
    const pet = new PetModel(march7th, 0);
    pet.acceptSample(sample(0), 0);
    for (let i = 0; i < 10; i++) {
      pet.acceptSample(sample(i + 1), i * 90);
      expect(pet.frameAt(i * 90, center)).toEqual({ row: 1, column: i % 8 });
    }
  });
  it("isolates state between instances", () => {
    const first = new PetModel(march7th, 0);
    const second = new PetModel(march7th, 0);
    first.beginDrag(0);
    expect(first.frameAt(0, center).row).toBe(1);
    expect(second.frameAt(0, center).row).toBe(0);
  });
  it("uses character-specific rows, frame count and cadence", () => {
    const alternate = {
      ...march7th,
      atlas: { ...march7th.atlas, columns: 4 },
      clips: { ...march7th.clips, idle: { row: 3, frameCount: 2, frameIntervalMs: 50 } },
      look: { firstRow: 6, directionCount: 8 },
    };
    const pet = new PetModel(alternate, 0);
    expect(pet.frameAt(49, center)).toEqual({ row: 3, column: 0 });
    expect(pet.frameAt(50, center)).toEqual({ row: 3, column: 1 });
    expect(pet.frameAt(100, center)).toEqual({ row: 3, column: 0 });
    expect(directionFrame({ x: -100, y: 0 }, center, 20, alternate)).toEqual({ row: 7, column: 2 });
  });
  it("maps every configured gaze direction, including wraparound", () => {
    for (let index = 0; index < 16; index++) {
      const angle = index * Math.PI / 8;
      const point = { x: Math.sin(angle) * 100, y: -Math.cos(angle) * 100 };
      expect(directionFrame(point, center, 20, march7th)).toEqual({ row: 9 + Math.floor(index / 8), column: index % 8 });
    }
    expect(directionFrame({ x: -0.01, y: -100 }, center, 20, march7th)).toEqual({ row: 9, column: 0 });
  });
  it("plays a one-shot from its first frame and catches up after a delayed frame", () => {
    const pet = new PetModel(march7th, 0);
    expect(pet.respond("wave", 100)).toBe(true);
    expect(pet.frameAt(100, center)).toEqual({ row: 3, column: 0 });
    expect(pet.frameAt(459, center)).toEqual({ row: 3, column: 1 });
    expect(pet.frameAt(640, center)).toEqual({ row: 3, column: 3 });
    expect(pet.frameAt(820, center).row).toBe(0);
  });
  it("offers water through four dedicated frames and holds the cup until the reminder ends", () => {
    const pet = new PetModel(march7th, 0);
    expect(pet.respond("offerWater", 100)).toBe(true);
    expect(pet.frameAt(100, center)).toEqual({ asset: "waterOffer", row: 0, column: 0 });
    expect(pet.frameAt(320, center)).toEqual({ asset: "waterOffer", row: 0, column: 1 });
    expect(pet.frameAt(540, center)).toEqual({ asset: "waterOffer", row: 1, column: 0 });
    expect(pet.frameAt(760, center)).toEqual({ asset: "waterOffer", row: 1, column: 1 });
    expect(pet.frameAt(8_000, center)).toEqual({ asset: "waterOffer", row: 1, column: 1 });
    pet.endOfferWater(8_000);
    expect(pet.frameAt(8_000, center)).toEqual({ asset: "waterOffer", row: 1, column: 1 });
    expect(pet.frameAt(8_220, center)).toEqual({ asset: "waterOffer", row: 1, column: 0 });
    expect(pet.frameAt(8_440, center)).toEqual({ asset: "waterOffer", row: 0, column: 1 });
    expect(pet.frameAt(8_660, center)).toEqual({ asset: "waterOffer", row: 0, column: 0 });
    expect(pet.frameAt(8_880, center).asset).toBeUndefined();
  });
  it("prioritizes real movement and aborts rather than queues interaction", () => {
    const pet = new PetModel(march7th, 0);
    pet.acceptSample(sample(0), 0);
    expect(pet.respond("jump", 0)).toBe(true);
    pet.acceptSample(sample(8), 10);
    expect(pet.frameAt(10, center)).toEqual({ row: 1, column: 0 });
    expect(pet.respond("wave", 20)).toBe(false);
    expect(pet.frameAt(170, center).row).not.toBe(3);
  });
  it("aborts interaction on drag and falls back to idle when an action is unavailable", () => {
    const withoutWave = { ...march7th, clips: { ...march7th.clips, wave: undefined } };
    const pet = new PetModel(withoutWave, 0);
    expect(pet.respond("wave", 0)).toBe(true);
    expect(pet.frameAt(0, center)).toEqual({ row: 0, column: 0 });
    expect(pet.respond("jump", 20)).toBe(true);
    pet.beginDrag(30);
    expect(pet.frameAt(30, center).row).toBe(1);
  });
  it("plays one idle cycle for a missing action before restoring gaze", () => {
    const withoutWave = { ...march7th, clips: { ...march7th.clips, wave: undefined } };
    const pet = new PetModel(withoutWave, 0);
    pet.acceptSample(sample(0), 0);
    expect(pet.frameAt(0, center)).toEqual({ row: 9, column: 4 });
    expect(pet.respond("wave", 100)).toBe(true);
    expect(pet.frameAt(100, center)).toEqual({ row: 0, column: 0 });
    expect(pet.frameAt(1_779, center)).toEqual({ row: 0, column: 5 });
    expect(pet.frameAt(1_780, center)).toEqual({ row: 9, column: 4 });
  });
});
