import { describe, expect, it } from "vitest";
import catalogData from "../characters/catalog.json";
import { characterCatalog, parseCharacterCatalog } from "../characters/catalog";
import { PetModel } from "./pet-model";

const center = { x: 0, y: 0 };
const sample = (windowX: number) => ({ x: 100, y: 0, windowX, windowY: 0 });

function thirtyFrameCatalog() {
  const raw = structuredClone(catalogData) as any;
  const pet = raw.characters[1];
  pet.atlas = { ...pet.atlas, columns: 30, rows: 11 };
  pet.clips = {
    idle: { row: 0, frameCount: 30, frameIntervalMs: 56 },
    movingRight: { row: 1, frameCount: 30, frameIntervalMs: 24 },
    movingLeft: { row: 2, frameCount: 30, frameIntervalMs: 24 },
    wave: { row: 3, frameCount: 30, frameIntervalMs: 27 },
    jump: { row: 4, frameCount: 30, frameIntervalMs: 24 },
  };
  pet.waterOffer = { kind: "mainAtlas", clip: { row: 5, frameCount: 30, frameIntervalMs: 37 } };
  pet.presentation = {
    neutralFrame: { row: 0, column: 0 },
    neutralHoldMs: 80,
    finishRunBeforeExit: true,
    movementTransitions: {
      right: { row: 6, frameCount: 30, frameIntervalMs: 8 },
      left: { row: 7, frameCount: 30, frameIntervalMs: 8 },
    },
  };
  return raw;
}

const character = () => parseCharacterCatalog(thirtyFrameCatalog()).characters[1];

describe("30-frame catalog contract", () => {
  it("accepts the 30x11 geometry and preserves the compiled legacy character", () => {
    const parsed = parseCharacterCatalog(thirtyFrameCatalog());
    expect(parsed.characters[0]).toEqual(characterCatalog.characters[0]);
    expect(parsed.characters[1].atlas).toMatchObject({ columns: 30, rows: 11, cellWidth: 192, cellHeight: 208 });
    expect(parsed.characters[1].look).toEqual({ firstRow: 9, directionCount: 16 });
    expect(parsed.characters[1].waterOffer).toEqual({ kind: "mainAtlas", clip: { row: 5, frameCount: 30, frameIntervalMs: 37 } });
  });

  it.each([
    (raw: any) => { raw.characters[1].waterOffer.clip.frameCount = 31; },
    (raw: any) => { raw.characters[1].presentation.movementTransitions.right.frameCount = 31; },
    (raw: any) => { raw.characters[1].presentation.neutralFrame.column = 30; },
    (raw: any) => { raw.characters[1].presentation.finishRunBeforeExit = "true"; },
  ])("rejects an out-of-bounds 30-frame definition", invalidate => {
    const raw = thirtyFrameCatalog();
    invalidate(raw);
    expect(() => parseCharacterCatalog(raw)).toThrow();
  });
});

describe("30-frame playback boundaries", () => {
  it("starts the left entry immediately for a left pointer hint, including after an earlier right drag", () => {
    for (const priorRightDrag of [false, true]) {
      const pet = new PetModel(character(), 0);
      if (priorRightDrag) { pet.beginDrag(0, "right"); pet.frameAt(1000, center); }
      const now = priorRightDrag ? 1000 : 0;
      pet.beginDrag(now, "left");
      expect(pet.frameAt(now, center)).toEqual({ row: 7, column: 0 });
      expect(pet.frameAt(now + 16, center)).toEqual({ row: 7, column: 2 });
      expect(pet.frameAt(now + 24, center)).toEqual({ row: 7, column: 3 });
      pet.acceptSample(sample(-10), now + 33);
      expect(pet.frameAt(now + 33, center)).toEqual({ row: 7, column: 4 });
    }
  });

  it("finishes the current running phase before reversing the shared entry endpoint", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(449.999, center)).toEqual({ row: 1, column: 8 });
    expect(pet.isMoving(450)).toBe(false);
    expect(pet.frameAt(450, center)).toEqual({ row: 1, column: 8 });
    expect(pet.frameAt(474, center)).toEqual({ row: 1, column: 9 });
    expect(pet.frameAt(978, center)).toEqual({ row: 1, column: 0 });
    expect(pet.frameAt(1002, center)).toEqual({ row: 6, column: 29 });
    expect(pet.frameAt(1241.999, center)).toEqual({ row: 6, column: 0 });
    expect(pet.frameAt(1242, center)).toEqual({ row: 0, column: 0 });
  });

  it("resumes a stopped step from its current phase when the same drag continues", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(474, center)).toEqual({ row: 1, column: 9 });
    pet.acceptSample(sample(40), 480);
    expect(pet.frameAt(480, center)).toEqual({ row: 1, column: 9 });
    expect(pet.frameAt(504, center)).toEqual({ row: 1, column: 10 });
  });

  it("keeps the old-direction run and entry rows until a nonzero-phase reversal reaches neutral", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(399.999, center)).toEqual({ row: 1, column: 6 });
    pet.acceptSample(sample(20), 400);
    expect(pet.frameAt(400, center)).toEqual({ row: 1, column: 6 });
    pet.acceptSample(sample(10), 433);
    expect(pet.frameAt(464, center)).toEqual({ row: 1, column: 0 });
    pet.acceptSample(sample(0), 466);
    expect(pet.frameAt(466.667, center)).toEqual({ row: 6, column: 29 });
    expect(pet.frameAt(546.667, center)).toEqual({ row: 7, column: 0 });
  });

  it("changes a stopping step into a bounded turn without replacing its current pose", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(450, center)).toEqual({ row: 1, column: 8 });
    pet.acceptSample(sample(20), 460);
    expect(pet.frameAt(460, center)).toEqual({ row: 1, column: 8 });
    pet.acceptSample(sample(10), 493);
    expect(pet.frameAt(521.334, center)).toEqual({ row: 6, column: 29 });
    pet.acceptSample(sample(0), 526);
    expect(pet.frameAt(601.334, center)).toEqual({ row: 7, column: 0 });
  });

  it("continues the visible phase if a reversal turns back before reaching the shared pose", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    pet.acceptSample(sample(20), 400);
    expect(pet.frameAt(409.999, center)).toEqual({ row: 1, column: 9 });
    pet.acceptSample(sample(30), 410);
    expect(pet.frameAt(410, center)).toEqual({ row: 1, column: 9 });
    expect(pet.frameAt(434, center)).toEqual({ row: 1, column: 10 });
  });

  it("resolves a completed entry before a later stop even when no frame was requested at entry end", () => {
    const immediate = new PetModel(character(), 0);
    const delayed = new PetModel(character(), 0);
    for (const pet of [immediate, delayed]) {
      pet.acceptSample(sample(0), 0);
      pet.acceptSample(sample(10), 10);
      pet.acceptSample(sample(20), 150);
    }
    expect(immediate.frameAt(250, center)).toEqual({ row: 1, column: 0 });
    expect(delayed.frameAt(320, center)).toEqual(immediate.frameAt(320, center));
    expect(delayed.frameAt(320, center)).toEqual({ row: 1, column: 2 });
    delayed.acceptSample(sample(10), 320);
    immediate.acceptSample(sample(10), 320);
    expect(delayed.frameAt(320, center)).toEqual(immediate.frameAt(320, center));
    expect(delayed.frameAt(320, center)).toEqual({ row: 1, column: 2 });
  });

  it.each(["wave", "jump", "offerWater"] as const)("returns a final-frame %s within160ms while movement samples continue", action => {
    const definition = character();
    const pet = new PetModel(definition, 0);
    pet.acceptSample(sample(0), 0);
    pet.respond(action, 0);
    const interval = action === "offerWater" ? 37 : definition.clips[action]!.frameIntervalMs;
    const row = action === "offerWater" ? 5 : definition.clips[action]!.row;
    const dragAt = 29 * interval + 1;
    const started: string[] = [];
    pet.respond("jump", dragAt, () => started.push("queued"));
    pet.beginDrag(dragAt);
    expect(pet.frameAt(dragAt, center)).toEqual({ row, column: 29 });
    pet.acceptSample(sample(10), dragAt + 33);
    pet.acceptSample(sample(20), dragAt + 66);
    expect(pet.frameAt(dragAt + 80, center)).toEqual({ row, column: 14 });
    pet.acceptSample(sample(30), dragAt + 99);
    pet.acceptSample(sample(40), dragAt + 132);
    expect(pet.frameAt(dragAt + 159.999, center)).toEqual({ row, column: 0 });
    expect(pet.frameAt(dragAt + 160, center)).toEqual({ row: 6, column: 0 });
    expect(started).toEqual([]);
  });

  it("returns all30 turn frames within80ms during a rapid reversal", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    expect(pet.frameAt(250, center)).toEqual({ row: 1, column: 0 });
    pet.acceptSample(sample(10), 260);
    expect(pet.frameAt(260, center)).toEqual({ row: 6, column: 29 });
    pet.acceptSample(sample(0), 293);
    pet.acceptSample(sample(-10), 326);
    expect(pet.frameAt(339.999, center)).toEqual({ row: 6, column: 0 });
    expect(pet.frameAt(340, center)).toEqual({ row: 7, column: 0 });
  });

  it("accelerates a30-frame normal exit when movement reverses during it", () => {
    const pet = new PetModel(character(), 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(1002, center)).toEqual({ row: 6, column: 29 });
    pet.acceptSample(sample(20), 1012);
    expect(pet.frameAt(1012, center)).toEqual({ row: 6, column: 28 });
    pet.acceptSample(sample(10), 1045);
    pet.acceptSample(sample(0), 1078);
    expect(pet.frameAt(1092, center)).toEqual({ row: 7, column: 0 });
  });

  it("retracts an early cup from14 and starts feedback only after its neutral hold", () => {
    const pet = new PetModel(character(), 0);
    pet.respond("offerWater", 0);
    expect(pet.frameAt(518, center)).toEqual({ row: 5, column: 14 });
    expect(pet.endOfferWater(519)).toBe(15 * 37 + 80);
    expect(pet.endOfferWater(520)).toBeNull();
    const started: string[] = [];
    pet.respond("jump", 520, () => started.push("feedback"));
    expect(pet.frameAt(519 + 14 * 37, center)).toEqual({ row: 5, column: 0 });
    expect(pet.frameAt(519 + 15 * 37, center)).toEqual({ row: 0, column: 0 });
    expect(started).toEqual([]);
    expect(pet.frameAt(519 + 15 * 37 + 80, center)).toEqual({ row: 4, column: 0 });
    expect(started).toEqual(["feedback"]);
  });

  it("holds a completed wave at neutral for80ms and keeps only the latest queued action", () => {
    const pet = new PetModel(character(), 0);
    const started: string[] = [];
    pet.respond("wave", 0);
    pet.respond("wave", 100, () => started.push("old"));
    pet.respond("jump", 200, () => started.push("latest"));
    expect(pet.frameAt(809, center)).toEqual({ row: 3, column: 29 });
    expect(pet.frameAt(810, center)).toEqual({ row: 0, column: 0 });
    expect(pet.frameAt(889, center)).toEqual({ row: 0, column: 0 });
    expect(pet.frameAt(890, center)).toEqual({ row: 4, column: 0 });
    expect(started).toEqual(["latest"]);
  });

  it("keeps every look direction in the sixteen occupied columns of row9", () => {
    const pet = new PetModel(character(), 0);
    for (let direction = 0; direction < 16; direction++) {
      const radians = direction * Math.PI / 8;
      pet.acceptSample({ x: 100 * Math.sin(radians), y: -100 * Math.cos(radians), windowX: 0, windowY: 0 }, direction);
      expect(pet.frameAt(direction, center)).toEqual({ row: 9, column: direction });
    }
  });
});
