import { describe, expect, it } from "vitest";
import { march7th } from "../characters/march-7th";
import { PetModel } from "./pet-model";
import { directionFrame } from "./animation";
import type { CharacterDefinition } from "./character";

const center = { x: 0, y: 0 };
const sample = (windowX: number, windowY = 0) => ({ x: 100, y: 0, windowX, windowY });

const sequencedCharacter: CharacterDefinition = {
  ...march7th,
  clips: { ...march7th.clips, wave: { row: 3, frameCount: 8, frameIntervalMs: 100 }, jump: { row: 4, frameCount: 8, frameIntervalMs: 90 } },
  waterOffer: { kind: "mainAtlas", clip: { row: 5, frameCount: 8, frameIntervalMs: 140 } },
  presentation: { neutralFrame: { row: 0, column: 0 }, neutralHoldMs: 80,
    movementTransitions: { right: { row: 6, frameCount: 4, frameIntervalMs: 60 }, left: { row: 7, frameCount: 4, frameIntervalMs: 60 } } },
};

const doubledCharacter: CharacterDefinition = {
  ...sequencedCharacter,
  atlas: { ...sequencedCharacter.atlas, columns: 16, rows: 10 },
  clips: {
    idle: { row: 0, frameCount: 12, frameIntervalMs: 140 },
    movingRight: { row: 1, frameCount: 16, frameIntervalMs: 45 },
    movingLeft: { row: 2, frameCount: 16, frameIntervalMs: 45 },
    wave: { row: 3, frameCount: 16, frameIntervalMs: 50 },
    jump: { row: 4, frameCount: 16, frameIntervalMs: 45 },
  },
  waterOffer: { kind: "mainAtlas", clip: { row: 5, frameCount: 16, frameIntervalMs: 70 } },
  presentation: { ...sequencedCharacter.presentation!,
    movementTransitions: { right: { row: 6, frameCount: 8, frameIntervalMs: 30 }, left: { row: 7, frameCount: 8, frameIntervalMs: 30 } } },
};

describe("doubled clips preserve interactive return budgets", () => {
  it.each(["wave", "jump", "offerWater"] as const)("returns all 16 %s frames within 160 ms during drag without extending deadlines or playing queued gestures", action => {
    const pet = new PetModel(doubledCharacter, 0);
    pet.acceptSample(sample(0), 0);
    pet.respond(action, 0);
    const interval = action === "offerWater" ? 70 : doubledCharacter.clips[action]!.frameIntervalMs;
    const dragAt = interval * 15;
    const actionRow = action === "offerWater" ? 5 : doubledCharacter.clips[action]!.row;
    const started: string[] = [];
    pet.respond("wave", dragAt, () => started.push("queued"));
    pet.beginDrag(dragAt);
    expect(pet.frameAt(dragAt, center)).toEqual({ row: actionRow, column: 15 });
    for (let step = 1; step <= 4; step++) pet.acceptSample(sample(step * 10), dragAt + step * 33);
    expect(pet.frameAt(dragAt + 150, center)).toEqual({ row: actionRow, column: 0 });
    expect(pet.frameAt(dragAt + 160, center)).toEqual({ row: 6, column: 0 });
    pet.frameAt(dragAt + 1000, center);
    expect(started).toEqual([]);
  });

  it("reverses all 8 turn frames within 80 ms while native movement continues", () => {
    const pet = new PetModel(doubledCharacter, 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(290, center)).toEqual({ row: 1, column: 0 });
    pet.acceptSample(sample(20), 300);
    expect(pet.frameAt(300, center)).toEqual({ row: 6, column: 7 });
    pet.acceptSample(sample(10), 333);
    pet.acceptSample(sample(0), 366);
    expect(pet.frameAt(370, center)).toEqual({ row: 6, column: 0 });
    expect(pet.frameAt(380, center)).toEqual({ row: 7, column: 0 });
  });
  it("accelerates a normal movement exit when the user changes direction during it", () => {
    const pet = new PetModel(doubledCharacter, 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(450, center)).toEqual({ row: 6, column: 7 });
    pet.acceptSample(sample(20), 460);
    expect(pet.frameAt(460, center)).toEqual({ row: 6, column: 7 });
    pet.acceptSample(sample(10), 493);
    pet.acceptSample(sample(0), 526);
    expect(pet.frameAt(530, center)).toEqual({ row: 6, column: 0 });
    expect(pet.frameAt(540, center)).toEqual({ row: 7, column: 0 });
  });
});

describe("opt-in sequenced playback", () => {
  it("starts a left drag in the left entry before a native position sample arrives", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.beginDrag(0, "left");
    expect(pet.frameAt(16, center)).toEqual({ row: 7, column: 0 });
    expect(pet.isMoving(159.999)).toBe(true);
    expect(pet.isMoving(160)).toBe(false);
  });

  it("keeps the last horizontal direction for a later pure vertical drag", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.beginDrag(0, "left");
    pet.frameAt(1000, center);
    pet.beginDrag(1000);
    expect(pet.frameAt(1000, center)).toEqual({ row: 7, column: 0 });
  });

  it("ignores the optional hint for the legacy character", () => {
    const pet = new PetModel(march7th, 0);
    pet.beginDrag(0, "left");
    expect(pet.frameAt(0, center)).toEqual({ row: 1, column: 0 });
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(-10), 33);
    expect(pet.frameAt(33, center)).toEqual({ row: 2, column: 0 });
  });

  it("uses the main atlas and reverses an early cup return from its current frame exactly once", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.respond("offerWater", 0);
    expect(pet.frameAt(280, center)).toEqual({ row: 5, column: 2 });
    pet.endOfferWater(280);
    pet.endOfferWater(300);
    expect(pet.frameAt(420, center)).toEqual({ row: 5, column: 1 });
    expect(pet.frameAt(560, center)).toEqual({ row: 5, column: 0 });
    expect(pet.frameAt(700, center)).toEqual({ row: 0, column: 0 });
  });
  it("finishes the current action, keeps only the latest pending action and holds neutral before restoring gaze", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.acceptSample(sample(0), 0);
    pet.respond("wave", 0);
    pet.respond("wave", 100);
    pet.respond("jump", 200);
    expect(pet.frameAt(200, center)).toEqual({ row: 3, column: 2 });
    expect(pet.frameAt(800, center)).toEqual({ row: 0, column: 0 });
    expect(pet.frameAt(880, center)).toEqual({ row: 4, column: 0 });
    expect(pet.frameAt(1600, center)).toEqual({ row: 0, column: 0 });
    expect(pet.frameAt(1680, center)).toEqual({ row: 9, column: 4 });
  });
  it("returns the cup before completion feedback regardless of whether end arrives first", () => {
    for (const order of ["endFirst", "responseFirst"]) {
      const pet = new PetModel(sequencedCharacter, 0);
      pet.respond("offerWater", 0);
      if (order === "endFirst") pet.endOfferWater(280);
      pet.respond("jump", 280);
      if (order === "responseFirst") pet.endOfferWater(280);
      expect(pet.frameAt(280, center)).toEqual({ row: 5, column: 2 });
      expect(pet.frameAt(700, center)).toEqual({ row: 0, column: 0 });
      expect(pet.frameAt(780, center)).toEqual({ row: 4, column: 0 });
    }
  });
  it("uses a fast return on drag, discards pending actions, and enters then exits running", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.respond("wave", 0);
    pet.respond("jump", 100);
    pet.beginDrag(200);
    expect(pet.frameAt(200, center)).toEqual({ row: 3, column: 2 });
    expect(pet.frameAt(220, center)).toEqual({ row: 3, column: 1 });
    expect(pet.frameAt(240, center)).toEqual({ row: 3, column: 0 });
    expect(pet.frameAt(260, center)).toEqual({ row: 6, column: 0 });
    pet.acceptSample(sample(0), 260);
    pet.acceptSample(sample(10), 300);
    pet.acceptSample(sample(20), 450);
    expect(pet.frameAt(500, center)).toEqual({ row: 1, column: 0 });
    expect(pet.frameAt(610, center)).toEqual({ row: 6, column: 3 });
    expect(pet.frameAt(850, center)).toEqual({ row: 0, column: 0 });
    expect(pet.frameAt(930, center).row).not.toBe(4);
  });
  it("keeps the latest horizontal direction during vertical movement", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.acceptSample(sample(10), 0);
    pet.acceptSample(sample(0), 1);
    expect(pet.frameAt(1, center)).toEqual({ row: 7, column: 0 });
    pet.acceptSample(sample(0, 10), 100);
    pet.acceptSample(sample(0, 20), 200);
    expect(pet.frameAt(241, center)).toEqual({ row: 2, column: 0 });
  });
  it("retracts from the current direction before entering a reverse run and resumes an interrupted exit", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.acceptSample(sample(0), 0);
    pet.acceptSample(sample(10), 10);
    pet.acceptSample(sample(20), 150);
    pet.acceptSample(sample(30), 290);
    expect(pet.frameAt(290, center)).toEqual({ row: 1, column: 0 });
    pet.acceptSample(sample(20), 300);
    expect(pet.frameAt(300, center)).toEqual({ row: 6, column: 3 });
    expect(pet.frameAt(360, center)).toEqual({ row: 6, column: 0 });
    expect(pet.frameAt(380, center)).toEqual({ row: 7, column: 0 });
    pet.acceptSample(sample(10), 400);
    pet.acceptSample(sample(0), 540);
    expect(pet.frameAt(620, center)).toEqual({ row: 2, column: 0 });
    expect(pet.frameAt(700, center)).toEqual({ row: 7, column: 3 });
    expect(pet.frameAt(760, center)).toEqual({ row: 7, column: 2 });
    pet.acceptSample(sample(-10), 760);
    expect(pet.frameAt(760, center)).toEqual({ row: 7, column: 2 });
    expect(pet.frameAt(820, center)).toEqual({ row: 7, column: 3 });
    expect(pet.frameAt(880, center)).toEqual({ row: 2, column: 0 });
  });
  it("does not start a cup that was ended while queued, and catches up neutral boundaries after a delayed frame", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    const started: string[] = [];
    pet.respond("wave", 0, () => started.push("wave"));
    pet.respond("offerWater", 100, () => started.push("cup"));
    pet.endOfferWater(200);
    expect(pet.frameAt(2000, center).row).toBe(0);
    expect(started).toEqual(["wave"]);
    pet.respond("wave", 2000);
    pet.respond("jump", 2100, () => started.push("jump"));
    expect(pet.frameAt(3060, center)).toEqual({ row: 4, column: 2 });
    expect(started).toEqual(["wave", "jump"]);
  });
  it("does not restart fast return timing on continuous native movement samples", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.acceptSample(sample(0), 0);
    pet.respond("wave", 0);
    pet.beginDrag(600);
    for (let step = 1; step <= 4; step++) pet.acceptSample(sample(step * 10), 600 + step * 33);
    expect(pet.frameAt(740, center)).toEqual({ row: 6, column: 0 });
    expect(pet.frameAt(800, center)).toEqual({ row: 6, column: 1 });
  });
  it("retains a newer cup request while retracting the previous cup", () => {
    const pet = new PetModel(sequencedCharacter, 0);
    pet.respond("offerWater", 0);
    expect(pet.respond("offerWater", 280)).toBe(true);
    expect(pet.frameAt(280, center)).toEqual({ row: 5, column: 2 });
    expect(pet.frameAt(780, center)).toEqual({ row: 5, column: 0 });
    expect(pet.frameAt(10000, center)).toEqual({ row: 5, column: 7 });
  });
});

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
