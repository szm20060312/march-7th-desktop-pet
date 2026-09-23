import { describe, expect, it } from "vitest";
import catalogData from "./catalog.json";
import { characterCatalog, defaultCharacter, parseCharacterCatalog } from "./catalog";

const responseContexts = ["click", "doubleClick", "reminderCompleted", "reminderSnoozed"] as const;

describe("compiled character catalog", () => {
  it("exposes the two reviewed characters from one envelope", () => {
    expect(characterCatalog.defaultId).toBe("march-7th");
    expect(characterCatalog.characters.map(character => [character.id, character.displayName])).toEqual([
      ["march-7th", "三月七"],
      ["raiden-shogun", "雷电将军"],
    ]);
    expect(defaultCharacter).toBe(characterCatalog.characters[0]);
  });

  it("keeps every declared frame inside the reviewed 8 by 11 atlas", () => {
    for (const character of characterCatalog.characters) {
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
    }
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

  it("rejects duplicate ids, unsafe paths, invalid frame ranges and unsafe phrases", () => {
    const clone = () => structuredClone(catalogData) as any;

    const duplicate = clone();
    duplicate.characters[1].id = duplicate.characters[0].id;
    duplicate.characters[1].atlas.src = duplicate.characters[0].atlas.src;
    expect(() => parseCharacterCatalog(duplicate)).toThrow(/duplicate/i);

    const unsafePath = clone();
    unsafePath.characters[0].atlas.src = "https://example.com/pet.webp";
    expect(() => parseCharacterCatalog(unsafePath)).toThrow(/atlas.*path/i);

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
