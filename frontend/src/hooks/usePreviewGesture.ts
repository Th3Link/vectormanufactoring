import { useCallback, useEffect, useRef, useState } from "react";

import type { WasmSession } from "@/lib/editorSession";

/** How a gesture ends in the session: the commit at the release and the drop at Escape. */
export interface GestureEnds {
  commit: (session: WasmSession) => void;
  cancel: (session: WasmSession) => void;
}

/** The Style panel's drag (the default). */
const STYLE_ENDS: GestureEnds = {
  commit: (session) => session.commit_style_preview(),
  cancel: (session) => session.cancel_style_preview(),
};

export interface PreviewGesture {
  /** Shows `tick` on the next frame; the first of a gesture starts it. */
  queue: (tick: (session: WasmSession) => void) => void;
  /** A gesture is running (from its first tick to its release). */
  previewing: boolean;
  /** Counts the Escapes that dropped a preview, so a colour picker that kept
   * following the pointer can start over from the committed colour. */
  cancels: number;
}

/**
 * The bookkeeping of one panel drag (`specs/0007-stroke-and-fill-styling`
 * criterion 36). Ticks are coalesced to one session update per animation
 * frame (the last one wins). The first tick of a gesture installs window
 * listeners, because a colour area reports neither its release nor its key-up:
 * a pointer release, the key-up of a key that steps a slider or a window blur commits once (the last
 * tick is flushed first, so the commit never lags a frame), and Escape drops
 * the preview, after which the release writes nothing. The listeners are
 * removed when the gesture ends or the panel goes away.
 *
 * `onChange` runs after every update the session took, so the panel re-reads
 * what it shows. `ends` says which session calls commit and drop the preview:
 * the Style panel's by default, the Background block's for the document
 * (`specs/0040-document-background` criterion 22). It must be a stable object.
 */
export function usePreviewGesture(
  getSession: () => WasmSession | null,
  onChange: () => void,
  ends: GestureEnds = STYLE_ENDS,
): PreviewGesture {
  const [previewing, setPreviewing] = useState(false);
  const [cancels, setCancels] = useState(0);
  const pending = useRef<((session: WasmSession) => void) | null>(null);
  const frame = useRef(0);
  const endGesture = useRef<(() => void) | null>(null);

  const dropPending = useCallback(() => {
    window.cancelAnimationFrame(frame.current);
    frame.current = 0;
    const tick = pending.current;
    pending.current = null;
    return tick;
  }, []);

  const flush = useCallback(() => {
    const tick = dropPending();
    const session = getSession();
    if (tick && session) {
      tick(session);
      onChange();
    }
  }, [dropPending, getSession, onChange]);

  /** The release: flushes the last tick, commits once, removes the listeners. */
  const commit = useCallback(() => {
    flush();
    endGesture.current?.();
    endGesture.current = null;
    setPreviewing(false);
    const session = getSession();
    if (session) {
      ends.commit(session);
    }
    onChange();
  }, [flush, getSession, onChange, ends]);

  const begin = useCallback(() => {
    if (endGesture.current) {
      return;
    }
    setPreviewing(true);
    const onRelease = () => commit();
    const onStepKeyUp = (event: KeyboardEvent) => {
      // The keys that step a slider: a held key is one gesture, one commit.
      if (event.key.startsWith("Arrow") || event.key === "Home" || event.key === "End") {
        commit();
      }
    };
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      // Escape drops the preview; it never reaches the popover or the canvas.
      event.preventDefault();
      event.stopPropagation();
      dropPending();
      const session = getSession();
      if (session) {
        ends.cancel(session);
      }
      setCancels((n) => n + 1);
      onChange();
    };
    // The system took the pointer away (`0017` criterion 40): the preview is
    // dropped and nothing is written, the same as Escape; the commit that ends
    // the gesture then has nothing to write.
    const onPointerCancel = () => {
      dropPending();
      const session = getSession();
      if (session) {
        ends.cancel(session);
      }
      commit();
    };
    window.addEventListener("pointerup", onRelease, true);
    window.addEventListener("pointercancel", onPointerCancel, true);
    window.addEventListener("blur", onRelease);
    window.addEventListener("keyup", onStepKeyUp, true);
    window.addEventListener("keydown", onEscape, true);
    endGesture.current = () => {
      window.removeEventListener("pointerup", onRelease, true);
      window.removeEventListener("pointercancel", onPointerCancel, true);
      window.removeEventListener("blur", onRelease);
      window.removeEventListener("keyup", onStepKeyUp, true);
      window.removeEventListener("keydown", onEscape, true);
    };
  }, [commit, dropPending, getSession, onChange, ends]);

  // A panel that goes away mid-gesture must not leave listeners behind.
  useEffect(
    () => () => {
      window.cancelAnimationFrame(frame.current);
      endGesture.current?.();
      endGesture.current = null;
    },
    [],
  );

  const queue = useCallback(
    (tick: (session: WasmSession) => void) => {
      begin();
      pending.current = tick;
      if (frame.current === 0) {
        frame.current = window.requestAnimationFrame(flush);
      }
    },
    [begin, flush],
  );
  return { queue, previewing, cancels };
}
