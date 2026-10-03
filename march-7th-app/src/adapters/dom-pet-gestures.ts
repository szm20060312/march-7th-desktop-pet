import type { PetGestureHandlers, PetGestureInput, Scheduler } from "../application/ports";

const DOUBLE_CLICK_MS = 300;
const DRAG_THRESHOLD_PX = 6;

type ActivePointer = { pointerId: number; x: number; y: number };
type Release = { x: number; y: number; at: number };

export function createDomPetGestures(
  stage: HTMLElement,
  documentTarget: Document,
  windowTarget: Window,
  scheduler: Scheduler,
): PetGestureInput {
  return {
    subscribe(handlers: PetGestureHandlers) {
      let active: ActivePointer | null = null;
      let previousRelease: Release | null = null;
      let clickTimer: number | undefined;
      let disposed = false;

      const cancelClick = () => {
        if (clickTimer !== undefined) scheduler.cancelDelay(clickTimer);
        clickTimer = undefined;
        previousRelease = null;
      };
      const releaseCapture = (pointerId: number) => {
        try {
          if (stage.hasPointerCapture(pointerId)) stage.releasePointerCapture(pointerId);
        } catch {
          // Capture may already have moved to the native window drag loop.
        }
      };
      const cancelActive = (pointerId?: number) => {
        if (active && (pointerId === undefined || pointerId === active.pointerId)) {
          const currentId = active.pointerId;
          active = null;
          releaseCapture(currentId);
        }
        cancelClick();
      };
      const down = (event: PointerEvent) => {
        if (disposed || event.button !== 0 || !event.isPrimary || active) return;
        active = { pointerId: event.pointerId, x: event.clientX, y: event.clientY };
        try { stage.setPointerCapture(event.pointerId); } catch { /* capture is best effort */ }
      };
      const move = (event: PointerEvent) => {
        if (!active || event.pointerId !== active.pointerId) return;
        if (Math.hypot(event.clientX - active.x, event.clientY - active.y) < DRAG_THRESHOLD_PX) return;
        const pointerId = active.pointerId;
        active = null;
        cancelClick();
        releaseCapture(pointerId);
        event.preventDefault();
        handlers.drag();
      };
      const up = (event: PointerEvent) => {
        if (!active || event.pointerId !== active.pointerId) return;
        const pointerId = active.pointerId;
        active = null;
        releaseCapture(pointerId);
        const now = scheduler.now();
        if (previousRelease && now - previousRelease.at <= DOUBLE_CLICK_MS &&
            Math.hypot(event.clientX - previousRelease.x, event.clientY - previousRelease.y) <= DRAG_THRESHOLD_PX) {
          cancelClick();
          handlers.doubleClick();
          return;
        }
        if (previousRelease) {
          cancelClick();
          handlers.click();
        }
        previousRelease = { x: event.clientX, y: event.clientY, at: now };
        if (clickTimer !== undefined) scheduler.cancelDelay(clickTimer);
        clickTimer = scheduler.setDelay(() => {
          clickTimer = undefined;
          previousRelease = null;
          if (!disposed) handlers.click();
        }, DOUBLE_CLICK_MS);
      };
      const cancelPointer = (event: PointerEvent) => {
        if (!active || event.pointerId !== active.pointerId) return;
        cancelActive(event.pointerId);
      };
      const blur = () => cancelActive();

      stage.addEventListener("pointerdown", down);
      documentTarget.addEventListener("pointermove", move);
      documentTarget.addEventListener("pointerup", up);
      documentTarget.addEventListener("pointercancel", cancelPointer);
      documentTarget.addEventListener("lostpointercapture", cancelPointer);
      stage.addEventListener("lostpointercapture", cancelPointer);
      windowTarget.addEventListener("blur", blur);

      return () => {
        if (disposed) return;
        disposed = true;
        cancelActive();
        stage.removeEventListener("pointerdown", down);
        documentTarget.removeEventListener("pointermove", move);
        documentTarget.removeEventListener("pointerup", up);
        documentTarget.removeEventListener("pointercancel", cancelPointer);
        documentTarget.removeEventListener("lostpointercapture", cancelPointer);
        stage.removeEventListener("lostpointercapture", cancelPointer);
        windowTarget.removeEventListener("blur", blur);
      };
    },
  };
}
