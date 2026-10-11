import type {
  AnimationClip,
  CharacterCatalog,
  CharacterDefinition,
  CharacterPresentation,
  WaterOffer,
  ReminderTopic,
  ResponseContext,
} from "../domain/character";
import data from "./catalog.json";

const clipNames = ["idle", "movingLeft", "movingRight", "wave", "jump"] as const;
const responseContexts = ["click", "doubleClick", "reminderDue", "reminderCompleted", "reminderSnoozed", "focusCompleted", "taskCompleted", "taskAbandoned"] as const;
const reminderTopics = ["water", "move", "eyes", "multiple"] as const;

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
  if (atlasData.pixelDensity !== undefined && atlasData.pixelDensity !== 1 && atlasData.pixelDensity !== 2) {
    throw new Error(`${path}.atlas.pixelDensity must be 1 or 2`);
  }
  const pixelDensity = atlasData.pixelDensity as 1 | 2 | undefined;

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

  const offerData = object(record.waterOffer, `${path}.waterOffer`);
  let waterOffer: WaterOffer;
  if (offerData.kind === "mainAtlas") {
    waterOffer = { kind: "mainAtlas", clip: parseClip(offerData.clip, `${path}.waterOffer.clip`, columns, rows) };
  } else {
    if (offerData.kind !== undefined && offerData.kind !== "separate") throw new Error(`${path}.waterOffer.kind is invalid`);
    const offerSrc = string(offerData.src, `${path}.waterOffer.src`);
    if (offerSrc !== `/assets/${id}/water-offer.png`) throw new Error(`${path}.waterOffer path must be a local built-in asset`);
    const sourceWidth = positiveInteger(offerData.sourceWidth, `${path}.waterOffer.sourceWidth`);
    const sourceHeight = positiveInteger(offerData.sourceHeight, `${path}.waterOffer.sourceHeight`);
    if (offerData.columns !== 2 || offerData.rows !== 2 || offerData.frameCount !== 4) throw new Error(`${path}.waterOffer must be a complete 2x2 action`);
    const offerInterval = positiveInteger(offerData.frameIntervalMs, `${path}.waterOffer.frameIntervalMs`);
    waterOffer = { ...(offerData.kind === "separate" ? { kind: "separate" as const } : {}), src: offerSrc, sourceWidth, sourceHeight, columns: 2, rows: 2, frameCount: 4, frameIntervalMs: offerInterval };
  }

  let presentation: CharacterPresentation | undefined;
  if (record.presentation !== undefined) {
    const settings = object(record.presentation, `${path}.presentation`);
    const neutral = object(settings.neutralFrame, `${path}.presentation.neutralFrame`);
    const row = nonNegativeInteger(neutral.row, `${path}.presentation.neutralFrame.row`);
    const column = nonNegativeInteger(neutral.column, `${path}.presentation.neutralFrame.column`);
    if (row >= rows || column >= columns) throw new Error(`${path}.presentation.neutralFrame exceeds atlas`);
    const transitions = object(settings.movementTransitions, `${path}.presentation.movementTransitions`);
    if (settings.finishRunBeforeExit !== undefined && typeof settings.finishRunBeforeExit !== "boolean") {
      throw new Error(`${path}.presentation.finishRunBeforeExit must be a boolean`);
    }
    presentation = {
      neutralFrame: { row, column },
      neutralHoldMs: positiveInteger(settings.neutralHoldMs, `${path}.presentation.neutralHoldMs`),
      movementTransitions: {
        left: parseClip(transitions.left, `${path}.presentation.movementTransitions.left`, columns, rows),
        right: parseClip(transitions.right, `${path}.presentation.movementTransitions.right`, columns, rows),
      },
      ...(settings.finishRunBeforeExit !== undefined ? { finishRunBeforeExit: settings.finishRunBeforeExit as boolean } : {}),
    };
  }

  const phraseData = object(record.phrases, `${path}.phrases`);
  let interaction: CharacterDefinition["interaction"];
  if (record.interaction !== undefined) {
    const settings = object(record.interaction, `${path}.interaction`);
    const region = object(settings.pointerRegion, `${path}.interaction.pointerRegion`);
    if (region.kind !== "visibleBounds") throw new Error(`${path}.interaction.pointerRegion.kind is invalid`);
    const paddingPx = nonNegativeInteger(region.paddingPx, `${path}.interaction.pointerRegion.paddingPx`);
    if (paddingPx > 12) throw new Error(`${path}.interaction.pointerRegion.paddingPx exceeds the allowed margin`);
    interaction = { pointerRegion: { kind: "visibleBounds", paddingPx } };
  }
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

  const promptData = object(record.reminderPrompts, `${path}.reminderPrompts`);
  const reminderPrompts = {} as Record<ReminderTopic, readonly string[]>;
  for (const topic of reminderTopics) {
    const values = promptData[topic];
    if (!Array.isArray(values) || values.length !== 2) throw new Error(`${path}.reminderPrompts.${topic} must contain two lines`);
    reminderPrompts[topic] = values.map((value: unknown, lineIndex: number) => {
      const line = string(value, `${path}.reminderPrompts.${topic}[${lineIndex}]`);
      if ([...line].length > 24 || /[\r\n<>]/.test(line)) throw new Error(`${path}.reminderPrompts.${topic} must be short plain text`);
      return line;
    });
  }

  return {
    id,
    displayName,
    atlas: { src, cellWidth, cellHeight, columns, rows, ...(pixelDensity !== undefined ? { pixelDensity } : {}) },
    clips: {
      idle: parsedClips.idle!,
      movingLeft: parsedClips.movingLeft!,
      movingRight: parsedClips.movingRight!,
      wave: parsedClips.wave,
      jump: parsedClips.jump,
    },
    look: { firstRow, directionCount },
    waterOffer,
    ...(presentation ? { presentation } : {}),
    ...(interaction ? { interaction } : {}),
    phrases,
    reminderPrompts,
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
