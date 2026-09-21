import { characterCatalog } from "./catalog";

/** Compatibility export. The catalog remains the only character data source. */
export const march7th = characterCatalog.characters.find(character => character.id === "march-7th")!;
