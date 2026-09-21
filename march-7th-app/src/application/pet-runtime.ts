import type { CharacterDefinition } from "../domain/character";
import { PetModel } from "../domain/pet-model";
import type { PetHost, PetView, Scheduler } from "./ports";

const SAMPLE_INTERVAL_MS = 33;

/** One lifecycle per call. stop() is idempotent and fences late native results. */
export function startPetRuntime(options: {
  character: CharacterDefinition;
  host: PetHost;
  view: PetView;
  scheduler: Scheduler;
  reportError: (operation: "drag", error: unknown) => void;
}): () => void {
  const { character, host, view, scheduler, reportError } = options;
  const model = new PetModel(character, scheduler.now());
  let stopped = false;
  let frameId: number | undefined;
  let timerId: number | undefined;

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

  const unsubscribe = view.onDragStart(() => {
    if (stopped) return;
    model.beginDrag(scheduler.now());
    // Native drag completion is not a reliable end-of-movement signal.
    void host.startDragging().catch(error => { if (!stopped) reportError("drag", error); });
  });

  frameId = scheduler.requestFrame(render);
  void sample();

  return () => {
    if (stopped) return;
    stopped = true;
    unsubscribe();
    if (frameId !== undefined) scheduler.cancelFrame(frameId);
    if (timerId !== undefined) scheduler.cancelDelay(timerId);
  };
}
