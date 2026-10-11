import { waterOfferClip, type CharacterDefinition, type OneShotAction, type ResponseContext } from "../domain/character";
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
  reportError: (operation: "drag" | "pointer-region", error: unknown) => void;
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
  let phraseVersion = 0;
  let reminderRequest: { ended: boolean } | null = null;
  const phraseIndexes: Partial<Record<ResponseContext, number>> = {};
  const hasPointerRegion = character.interaction?.pointerRegion.kind === "visibleBounds" &&
    !!host.setPointerPassthrough && !!view.containsPointer && !!view.setPointerRegionEnabled;
  let pointerRegionActive = hasPointerRegion;
  let appliedPassthrough: boolean | undefined;
  let requestedPassthrough: boolean | undefined;

  view.configure(character);
  if (hasPointerRegion) view.setPointerRegionEnabled!(true);
  view.render({ row: character.clips.idle.row, column: 0 });

  const restorePointerInput = (disableRegion = true) => {
    if (!hasPointerRegion) return;
    if (disableRegion) view.setPointerRegionEnabled!(false);
    void host.setPointerPassthrough!(false, character.id, false).catch(error => {
      if (!stopped) reportError("pointer-region", error);
    });
  };

  const updatePointerRegion = async (point: { x: number; y: number }) => {
    if (stopped || !pointerRegionActive) return;
    const ignore = !view.containsPointer!(point);
    if (requestedPassthrough === ignore && appliedPassthrough === ignore) return;
    requestedPassthrough = ignore;
    try {
      const applied = await host.setPointerPassthrough!(ignore, character.id);
      if (!stopped) appliedPassthrough = applied;
    } catch (error) {
      if (stopped) return;
      pointerRegionActive = false;
      reportError("pointer-region", error);
      restorePointerInput();
    }
  };

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
      await updatePointerRegion(cursor);
    } catch {
      if (stopped) return;
      model.sampleUnavailable();
      view.setTracking("unavailable");
      if (pointerRegionActive) { appliedPassthrough = undefined; requestedPassthrough = undefined; restorePointerInput(false); }
    } finally {
      if (!stopped) timerId = scheduler.setDelay(() => { void sample(); }, SAMPLE_INTERVAL_MS);
    }
  };

  const clearPhrase = () => {
    phraseVersion++;
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
    const action = actionOverride ?? actions[context];
    const reminder = context === "reminderDue" ? { ended: false } : null;
    const started = () => {
      if (stopped) return;
      if (character.presentation && reminder?.ended) { clearPhrase(); return; }
      const phrases = character.phrases[context];
      if (!phraseOverride && !phrases?.length) {
        clearPhrase();
        return;
      }
      const index = phraseIndexes[context] ?? 0;
      if (!phraseOverride && phrases?.length) phraseIndexes[context] = (index + 1) % phrases.length;
      if (phraseTimerId !== undefined) scheduler.cancelDelay(phraseTimerId);
      phraseTimerId = undefined;
      phraseVisible = true;
      phraseContext = context;
      const version = ++phraseVersion;
      view.showPhrase(phraseOverride ?? phrases![index]);
      if (context !== "reminderDue") phraseTimerId = scheduler.setDelay(() => {
        if (stopped || !phraseVisible || version !== phraseVersion) return;
        phraseTimerId = undefined;
        phraseVisible = false;
        view.clearPhrase();
      }, PHRASE_DURATION_MS);
    };
    const accepted = model.respond(action, now, started);
    // The native event adapter deduplicates authoritative response revisions.
    // Separate manual operations remain valid even without another automatic prompt.
    if (accepted && context === "reminderDue") reminderRequest = reminder;
    return accepted;
  };

  const unsubscribe = gestures.subscribe({
    click: () => { if (!options.onPetClick?.()) respond("click"); },
    doubleClick: () => { if (!options.onPetClick?.()) respond("doubleClick"); },
    drag: directionHint => {
      if (stopped) return;
      clearPhrase();
      model.beginDrag(scheduler.now(), directionHint);
      // Native drag completion is not a reliable end-of-movement signal.
      void host.startDragging().catch(error => { if (!stopped) reportError("drag", error); });
    },
  }, hasPointerRegion ? { accepts: point => !pointerRegionActive || view.containsPointer!(point) } : undefined);

  frameId = scheduler.requestFrame(render);
  void sample();

  const stop = () => {
    if (stopped) return;
    stopped = true;
    unsubscribe();
    pointerRegionActive = false;
    restorePointerInput();
    if (frameId !== undefined) scheduler.cancelFrame(frameId);
    if (timerId !== undefined) scheduler.cancelDelay(timerId);
    clearPhrase();
  };

  const endReminder = () => {
    if (stopped) return;
    if (character.presentation && reminderRequest?.ended) return;
    if (reminderRequest) reminderRequest.ended = true;
    const duration = model.endOfferWater(scheduler.now());
    if (phraseContext !== "reminderDue") return;
    if (phraseTimerId !== undefined) scheduler.cancelDelay(phraseTimerId);
    const version = phraseVersion;
    phraseTimerId = scheduler.setDelay(() => {
      if (stopped || version !== phraseVersion || phraseContext !== "reminderDue") return;
      phraseTimerId = undefined;
      clearPhrase();
    }, duration ?? waterOfferClip(character.waterOffer).frameCount * waterOfferClip(character.waterOffer).frameIntervalMs);
  };

  return { stop, respond, endReminder };
}
