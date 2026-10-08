import { useCallback, useEffect, useRef } from "react";

import type { WasmSession } from "@/lib/editorSession";

/**
 * The bookkeeping of one panel drag (`specs/0007-stroke-and-fill-styling`
 * criterion 36). Ticks are coalesced to one session update per animation
 * frame (the last one wins). The first tick of a gesture installs window
 * listeners, because a colour area reports neither its release nor its key-up:
 * a pointer release, an arrow key-up or a window blur commits once (the last
 * tick is flushed first, so the commit never lags a frame), and Escape drops
 * the preview, after which the release writes nothing. The listeners are
 * removed when the gesture ends or the panel goes away.
 *
 * `onChange` runs after every update the session took, so the panel re-reads
 * what it shows.
 */
export function usePreviewGesture(
  getSession: () => WasmSession | null,
  onChange: () => void,
): (tick: (session: WasmSession) => void) => void {
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
    getSession()?.commit_style_preview();
    onChange();
  }, [flush, getSession, onChange]);

  const begin = useCallback(() => {
    if (endGesture.current) {
      return;
    }
    const onRelease = () => commit();
    const onArrowUp = (event: KeyboardEvent) => {
      if (event.key.startsWith("Arrow")) {
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
      getSession()?.cancel_style_preview();
      onChange();
    };
    window.addEventListener("pointerup", onRelease, true);
    window.addEventListener("pointercancel", onRelease, true);
    window.addEventListener("blur", onRelease);
    window.addEventListener("keyup", onArrowUp, true);
    window.addEventListener("keydown", onEscape, true);
    endGesture.current = () => {
      window.removeEventListener("pointerup", onRelease, true);
      window.removeEventListener("pointercancel", onRelease, true);
      window.removeEventListener("blur", onRelease);
      window.removeEventListener("keyup", onArrowUp, true);
      window.removeEventListener("keydown", onEscape, true);
    };
  }, [commit, dropPending, getSession, onChange]);

  // A panel that goes away mid-gesture must not leave listeners behind.
  useEffect(
    () => () => {
      window.cancelAnimationFrame(frame.current);
      endGesture.current?.();
      endGesture.current = null;
    },
    [],
  );

  return useCallback(
    (tick) => {
      begin();
      pending.current = tick;
      if (frame.current === 0) {
        frame.current = window.requestAnimationFrame(flush);
      }
    },
    [begin, flush],
  );
}
