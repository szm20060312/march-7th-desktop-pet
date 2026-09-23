import type {
  AnimationClip,
  CharacterCatalog,
  CharacterDefinition,
  ResponseContext,
} from "../domain/character";
import data from "./catalog.json";

const clipNames = ["idle", "movingLeft", "movingRight", "wave", "jump"] as const;
const responseContexts = ["click", "doubleClick", "reminderCompleted", "reminderSnoozed", "focusCompleted"] as const;

function object(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${path} must be an object`);
  return value as Record<string, unknown>;
}

function string(value: unknown, path: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${path} must be a non-empty string`);
  return value;
}

function positiveInteger(value: unknown, path: string): number {
  if (!Number.isInteger(value) || (value as number) <= 0) throw new Error(`${path} must be a positive integer`);
  return value as number;
}

function nonNegativeInteger(value: unknown, path: string): number {
  if (!Number.isInteger(value) || (value as number) < 0) throw new Error(`${path} must be a non-negative integer`);
  return value as number;
}

function parseClip(value: unknown, path: string, columns: number, rows: number): AnimationClip {
  const record = object(value, path);
  const row = nonNegativeInteger(record.row, `${path}.row`);
  const frameCount = positiveInteger(record.frameCount, `${path}.frameCount`);
  const frameIntervalMs = positiveInteger(record.frameIntervalMs, `${path}.frameIntervalMs`);
  if (row >= rows) throw new Error(`${path}.row exceeds atlas rows`);
  if (frameCount > columns) throw new Error(`${path}.frameCount exceeds atlas columns`);
  return { row, frameCount, frameIntervalMs };
}

function parseCharacter(value: unknown, index: number): CharacterDefinition {
  const path = `characters[${index}]`;
  const record = object(value, path);
  const id = string(record.id, `${path}.id`);
  const displayName = string(record.displayName, `${path}.displayName`);
  const atlasData = object(record.atlas, `${path}.atlas`);
  const src = string(atlasData.src, `${path}.atlas.src`);
  if (src !== `/assets/${id}/spritesheet.webp`) {
    throw new Error(`${path}.atlas path must be a local built-in spritesheet`);
  }
  const cellWidth = positiveInteger(atlasData.cellWidth, `${path}.atlas.cellWidth`);
  const cellHeight = positiveInteger(atlasData.cellHeight, `${path}.atlas.cellHeight`);
  const columns = positiveInteger(atlasData.columns, `${path}.atlas.columns`);
  const rows = positiveInteger(atlasData.rows, `${path}.atlas.rows`);

  const clipData = object(record.clips, `${path}.clips`);
  const parsedClips: Partial<Record<(typeof clipNames)[number], AnimationClip>> = {};
  for (const name of clipNames) {
    if (clipData[name] !== undefined) parsedClips[name] = parseClip(clipData[name], `${path}.clips.${name}`, columns, rows);
  }
  for (const required of ["idle", "movingLeft", "movingRight"] as const) {
    if (!parsedClips[required]) throw new Error(`${path}.clips.${required} is required`);
  }

  const lookData = object(record.look, `${path}.look`);
  const firstRow = nonNegativeInteger(lookData.firstRow, `${path}.look.firstRow`);
  const directionCount = positiveInteger(lookData.directionCount, `${path}.look.directionCount`);
  if (firstRow + Math.ceil(directionCount / columns) > rows) throw new Error(`${path}.look exceeds atlas rows`);

  const phraseData = object(record.phrases, `${path}.phrases`);
  const phrases: Partial<Record<ResponseContext, readonly string[]>> = {};
  for (const context of responseContexts) {
    const values = phraseData[context];
    if (values === undefined) continue;
    if (!Array.isArray(values) || values.length !== 2) throw new Error(`${path}.phrases.${context} must contain two phrases`);
    phrases[context] = values.map((value, phraseIndex) => {
      const phrase = string(value, `${path}.phrases.${context}[${phraseIndex}]`);
      if ([...phrase].length > 14 || /[\r\n<>]/.test(phrase)) throw new Error(`${path}.phrase must be single-line plain text with at most 14 characters`);
      return phrase;
    });
  }

  return {
    id,
    displayName,
    atlas: { src, cellWidth, cellHeight, columns, rows },
    clips: {
      idle: parsedClips.idle!,
      movingLeft: parsedClips.movingLeft!,
      movingRight: parsedClips.movingRight!,
      wave: parsedClips.wave,
      jump: parsedClips.jump,
    },
    look: { firstRow, directionCount },
    phrases,
  };
}

export function parseCharacterCatalog(value: unknown): CharacterCatalog {
  const record = object(value, "catalog");
  const defaultId = string(record.defaultId, "catalog.defaultId");
  if (!Array.isArray(record.characters) || record.characters.length === 0) throw new Error("catalog.characters must not be empty");
  const characters = record.characters.map(parseCharacter);
  const ids = new Set<string>();
  for (const character of characters) {
    if (ids.has(character.id)) throw new Error(`duplicate character id: ${character.id}`);
    ids.add(character.id);
  }
  if (!ids.has(defaultId)) throw new Error(`catalog.defaultId does not exist: ${defaultId}`);
  return { defaultId, characters };
}

export const characterCatalog = parseCharacterCatalog(data);
export const defaultCharacter = characterCatalog.characters.find(character => character.id === characterCatalog.defaultId)!;
