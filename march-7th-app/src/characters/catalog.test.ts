import { describe, expect, it } from "vitest";
import catalogData from "./catalog.json";
import { characterCatalog, defaultCharacter, parseCharacterCatalog } from "./catalog";

const responseContexts = ["click", "doubleClick", "reminderDue", "reminderCompleted", "reminderSnoozed"] as const;

describe("compiled character catalog", () => {
  it("opts only Raiden into loading-safe reminders and water during focus", () => {
    expect((characterCatalog.characters[0] as any).reminderPresentation).toBeUndefined();
    expect((characterCatalog.characters[1] as any).reminderPresentation).toEqual({ deferUntilReady: true, waterDuringFocus: true, waterHoldMs: 300_000 });
    for (const invalid of [{ deferUntilReady: "yes", waterDuringFocus: true }, { deferUntilReady: true, waterDuringFocus: 1 }]) {
      const data = structuredClone(catalogData) as any;
      data.characters[1].reminderPresentation = invalid;
      expect(() => parseCharacterCatalog(data)).toThrow(/reminderPresentation/);
    }
    for (const waterHoldMs of [0, 119_999, 300_001, 300_000.5, "300000"]) {
      const data = structuredClone(catalogData) as any;
      data.characters[1].reminderPresentation.waterHoldMs = waterHoldMs;
      expect(() => parseCharacterCatalog(data)).toThrow(/waterHoldMs/);
    }
  });

  it("enables the compact pointer region only for Raiden without changing March 7th", () => {
    expect(characterCatalog.characters[0].interaction).toBeUndefined();
    expect(characterCatalog.characters[1].interaction).toEqual({ pointerRegion: { kind: "visibleBounds", paddingPx: 4 } });
    for (const pointerRegion of [{ kind: "unknown", paddingPx: 4 }, { kind: "visibleBounds", paddingPx: -1 }, { kind: "visibleBounds", paddingPx: 13 }]) {
      const data = structuredClone(catalogData) as any;
      data.characters[1].interaction = { pointerRegion };
      expect(() => parseCharacterCatalog(data)).toThrow(/pointerRegion/);
    }
  });
  it("accepts a bounded main-atlas cup and opt-in transitions without changing legacy entries", () => {
    const data = structuredClone(catalogData) as any;
    data.characters[1].waterOffer = { kind: "mainAtlas", clip: { row: 5, frameCount: 8, frameIntervalMs: 140 } };
    data.characters[1].presentation = { neutralFrame: { row: 0, column: 0 }, neutralHoldMs: 80,
      movementTransitions: { left: { row: 7, frameCount: 4, frameIntervalMs: 60 }, right: { row: 6, frameCount: 4, frameIntervalMs: 60 } } };
    const parsed = parseCharacterCatalog(data);
    expect(parsed.characters[0]).toEqual(characterCatalog.characters[0]);
    expect(parsed.characters[1].waterOffer).toEqual(data.characters[1].waterOffer);
    expect(parsed.characters[1].presentation).toEqual(data.characters[1].presentation);
    for (const invalid of [
      (value: any) => { value.characters[1].waterOffer.clip.row = 11; },
      (value: any) => { value.characters[1].waterOffer.clip.frameCount = value.characters[1].atlas.columns + 1; },
      (value: any) => { value.characters[1].presentation.neutralFrame.column = value.characters[1].atlas.columns; },
      (value: any) => { value.characters[1].presentation.movementTransitions.left.row = 11; },
      (value: any) => { value.characters[1].waterOffer.kind = "external"; },
    ]) {
      const changed = structuredClone(data); invalid(changed);
      expect(() => parseCharacterCatalog(changed)).toThrow();
    }
  });

  it("also accepts an explicitly separate built-in cup without weakening local path rules", () => {
    const data = structuredClone(catalogData) as any;
    data.characters[0].waterOffer.kind = "separate";
    expect(parseCharacterCatalog(data).characters[0].waterOffer).toMatchObject({ kind: "separate", src: "/assets/march-7th/water-offer.png" });
    data.characters[0].waterOffer.src = "file:///outside.png";
    expect(() => parseCharacterCatalog(data)).toThrow(/path/);
  });
  it("exposes the two reviewed characters from one envelope", () => {
    expect(characterCatalog.defaultId).toBe("march-7th");
    expect(characterCatalog.characters.map(character => [character.id, character.displayName])).toEqual([
      ["march-7th", "三月七"],
      ["raiden-shogun", "雷电将军"],
    ]);
    expect(defaultCharacter).toBe(characterCatalog.characters[0]);
  });

  it("preserves the reviewed legacy March 7th clips and separate cup", () => {
    for (const character of characterCatalog.characters.filter(item => item.id === "march-7th")) {
      expect(character.atlas).toMatchObject({ cellWidth: 192, cellHeight: 208, columns: 8, rows: 11 });
      expect(character.clips).toEqual({
        idle: { row: 0, frameCount: 6, frameIntervalMs: 280 },
        movingRight: { row: 1, frameCount: 8, frameIntervalMs: 90 },
        movingLeft: { row: 2, frameCount: 8, frameIntervalMs: 90 },
        wave: { row: 3, frameCount: 4, frameIntervalMs: 180 },
        jump: { row: 4, frameCount: 5, frameIntervalMs: 120 },
      });
      expect(character.look).toEqual({ firstRow: 9, directionCount: 16 });
      expect(Object.values(character.clips).every(clip => clip.row < 5)).toBe(true);
      expect(character.waterOffer).toMatchObject({ src: `/assets/${character.id}/water-offer.png`, columns: 2, rows: 2, frameCount: 4, frameIntervalMs: 220 });
    }
  });

  it("uses all eight 30-frame Raiden actions and sixteen gaze directions in the atlas", () => {
    const character = characterCatalog.characters.find(item => item.id === "raiden-shogun")!;
    expect(character.atlas).toMatchObject({ cellWidth: 192, cellHeight: 208, columns: 30, rows: 11 });
    expect(character.look).toEqual({ firstRow: 9, directionCount: 16 });
    expect(character.clips).toEqual({
      idle: { row: 0, frameCount: 30, frameIntervalMs: 56 },
      movingRight: { row: 1, frameCount: 30, frameIntervalMs: 24 },
      movingLeft: { row: 2, frameCount: 30, frameIntervalMs: 24 },
      wave: { row: 3, frameCount: 30, frameIntervalMs: 27 },
      jump: { row: 4, frameCount: 30, frameIntervalMs: 24 },
    });
    expect(character.waterOffer).toEqual({ kind: "mainAtlas", clip: { row: 5, frameCount: 30, frameIntervalMs: 37 } });
    expect(character.presentation?.movementTransitions).toEqual({
      left: { row: 7, frameCount: 30, frameIntervalMs: 8 },
      right: { row: 6, frameCount: 30, frameIntervalMs: 8 },
    });
    const actions = [...Object.values(character.clips), character.waterOffer.kind === "mainAtlas" ? character.waterOffer.clip : null,
      ...Object.values(character.presentation?.movementTransitions ?? {})].filter(clip => clip !== null);
    expect(actions).toHaveLength(8);
    expect(actions.reduce((count, clip) => count + clip.frameCount, character.look.directionCount)).toBe(256);
    for (const clip of actions) { expect(clip.frameCount).toBeLessThanOrEqual(character.atlas.columns); expect(clip.row).toBeLessThan(character.atlas.rows); }
  });

  it("provides two safe, short, deterministic phrases per context", () => {
    for (const character of characterCatalog.characters) {
      for (const context of responseContexts) {
        const phrases = character.phrases[context];
        expect(phrases).toHaveLength(2);
        for (const phrase of phrases ?? []) {
          expect([...phrase].length).toBeLessThanOrEqual(14);
          expect(phrase).not.toMatch(/[\r\n<>]/);
        }
      }
    }
  });

  it("gives both characters distinct original lines for each reminder topic", () => {
    for (const character of characterCatalog.characters) {
      for (const topic of ["water", "move", "eyes", "multiple"] as const) {
        expect(character.reminderPrompts[topic]).toHaveLength(2);
        for (const line of character.reminderPrompts[topic]) {
          expect([...line].length).toBeLessThanOrEqual(24);
          expect(line).not.toMatch(/[\r\n<>]/);
        }
      }
    }
    expect(characterCatalog.characters[0].reminderPrompts.water[0]).not.toBe(characterCatalog.characters[1].reminderPrompts.water[0]);
  });

  it("rejects duplicate ids, unsafe paths, invalid frame ranges and unsafe phrases", () => {
    const clone = () => structuredClone(catalogData) as any;

    const duplicate = clone();
    duplicate.characters[1].id = duplicate.characters[0].id;
    duplicate.characters[1].atlas.src = duplicate.characters[0].atlas.src;
    duplicate.characters[1].waterOffer.src = duplicate.characters[0].waterOffer.src;
    expect(() => parseCharacterCatalog(duplicate)).toThrow(/duplicate/i);

    const unsafePath = clone();
    unsafePath.characters[0].atlas.src = "https://example.com/pet.webp";
    expect(() => parseCharacterCatalog(unsafePath)).toThrow(/atlas.*path/i);

    const unsafeOffer = clone();
    unsafeOffer.characters[0].waterOffer.src = "https://example.com/offer.png";
    expect(() => parseCharacterCatalog(unsafeOffer)).toThrow(/waterOffer.*path/i);

    const outsideAtlas = clone();
    outsideAtlas.characters[0].clips.wave.frameCount = 9;
    expect(() => parseCharacterCatalog(outsideAtlas)).toThrow(/frameCount/i);

    const invalidLook = clone();
    invalidLook.characters[0].look.firstRow = 10;
    expect(() => parseCharacterCatalog(invalidLook)).toThrow(/look/i);

    const multiline = clone();
    multiline.characters[0].phrases.click[0] = "第一行\n第二行";
    expect(() => parseCharacterCatalog(multiline)).toThrow(/phrase/i);

    const html = clone();
    html.characters[0].phrases.click[0] = "<b>你好</b>";
    expect(() => parseCharacterCatalog(html)).toThrow(/phrase/i);
  });
});

it("gives both built-in characters two gentle focus completion phrases using the existing context", () => {
  for (const character of characterCatalog.characters) expect(character.phrases.focusCompleted).toHaveLength(2);
});
