import type { CharacterDefinition, ReminderTopic } from "./character";
import type { ReminderId } from "./reminder";

export function reminderPrompt(character: CharacterDefinition, items: readonly ReminderId[], presentationId: number): string | null {
  if (!items.length || !Number.isSafeInteger(presentationId) || presentationId < 1) return null;
  const topic: ReminderTopic = items.length > 1 ? "multiple" : items[0];
  const lines = character.reminderPrompts[topic];
  return lines[(presentationId - 1) % lines.length] ?? null;
}
