import type { CharacterDefinition, OneShotAction, ResponseContext } from "../domain/character";
import { PetModel } from "../domain/pet-model";
import type { PetGestureInput, PetHost, PetView, Scheduler } from "./ports";

const SAMPLE_INTERVAL_MS = 33;
const PHRASE_DURATION_MS = 3_000;
const actions: Readonly<Record<ResponseContext, OneShotAction>> = {
  click: "wave",
  doubleClick: "jump",
  reminderDue: "wave",
  reminderCompleted: "jump",
  reminderSnoozed: "wave",
  focusCompleted: "wave",
  taskCompleted: "wave",
  taskAbandoned: "wave",
};

/** One lifecycle per call. stop() is idempotent and fences late native results. */
export function startPetRuntime(options: {
  character: CharacterDefinition;
  host: PetHost;
  view: PetView;
  gestures: PetGestureInput;
  scheduler: Scheduler;
  reportError: (operation: "drag", error: unknown) => void;
  onPetClick?: () => boolean;
}) {
  const { character, host, view, gestures, scheduler, reportError } = options;
  const model = new PetModel(character, scheduler.now());
  let stopped = false;
  let frameId: number | undefined;
  let timerId: number | undefined;
  let phraseTimerId: number | undefined;
  let phraseVisible = false;
  let phraseContext: ResponseContext | null = null;
  const phraseIndexes: Partial<Record<ResponseContext, number>> = {};

  view.configure(character);
  view.render({ row: character.clips.idle.row, column: 0 });

  const render = (now: number) => {
    if (stopped) return;
    view.render(model.frameAt(now, view.center()));
    frameId = scheduler.requestFrame(render);
  };

  const sample = async () => {
    if (stopped) return;
    try {
      const cursor = await host.sampleCursor();
      if (stopped) return;
      const now = scheduler.now();
      model.acceptSample(cursor, now);
      view.setTracking(model.isMoving(now) ? "moving" : "active");
    } catch {
      if (stopped) return;
      model.sampleUnavailable();
      view.setTracking("unavailable");
    } finally {
      if (!stopped) timerId = scheduler.setDelay(() => { void sample(); }, SAMPLE_INTERVAL_MS);
    }
  };

  const clearPhrase = () => {
    if (phraseTimerId !== undefined) scheduler.cancelDelay(phraseTimerId);
    phraseTimerId = undefined;
    phraseContext = null;
    if (!phraseVisible) return;
    phraseVisible = false;
    view.clearPhrase();
  };

  const respond = (context: ResponseContext, phraseOverride?: string, actionOverride?: OneShotAction): boolean => {
    if (stopped) return false;
    const now = scheduler.now();
    if (!model.respond(actionOverride ?? actions[context], now)) return false;
    const phrases = character.phrases[context];
    if (!phraseOverride && !phrases?.length) {
      clearPhrase();
      return true;
    }
    const index = phraseIndexes[context] ?? 0;
    if (!phraseOverride && phrases?.length) phraseIndexes[context] = (index + 1) % phrases.length;
    if (phraseTimerId !== undefined) scheduler.cancelDelay(phraseTimerId);
    phraseVisible = true;
    phraseContext = context;
    view.showPhrase(phraseOverride ?? phrases![index]);
    phraseTimerId = scheduler.setDelay(() => {
      phraseTimerId = undefined;
      if (stopped || !phraseVisible) return;
      phraseVisible = false;
      view.clearPhrase();
    }, PHRASE_DURATION_MS);
    return true;
  };

  const unsubscribe = gestures.subscribe({
    click: () => { if (!options.onPetClick?.()) respond("click"); },
    doubleClick: () => { if (!options.onPetClick?.()) respond("doubleClick"); },
    drag: () => {
      if (stopped) return;
      clearPhrase();
      model.beginDrag(scheduler.now());
      // Native drag completion is not a reliable end-of-movement signal.
      void host.startDragging().catch(error => { if (!stopped) reportError("drag", error); });
    },
  });

  frameId = scheduler.requestFrame(render);
  void sample();

  const stop = () => {
    if (stopped) return;
    stopped = true;
    unsubscribe();
    if (frameId !== undefined) scheduler.cancelFrame(frameId);
    if (timerId !== undefined) scheduler.cancelDelay(timerId);
    clearPhrase();
  };

  const endReminder = () => {
    model.endOfferWater();
    if (phraseContext === "reminderDue") clearPhrase();
  };

  return { stop, respond, endReminder };
}
