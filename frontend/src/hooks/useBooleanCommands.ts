import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { type ActionNotice, useActionNotice } from "@/hooks/useActionNotice";
import type { EditorSession } from "@/hooks/useEditorSession";
import {
  type BooleanAvailabilityState,
  type BooleanOp,
  refusalOutlinesObjects,
  refusalText,
  successText,
} from "@/lib/booleanText";

export interface BooleanCommands {
  availability: BooleanAvailabilityState;
  /** A kernel call is about to run or running: wait cursor, input ignored. */
  busy: boolean;
  notice: ActionNotice | null;
  /** Runs `op` on the selection. Does nothing while dimmed or busy. */
  apply: (op: BooleanOp) => void;
}

/** Resolves after `count` painted frames. */
function afterFrames(count: number): Promise<void> {
  return new Promise((resolve) => {
    const step = (left: number) => {
      if (left <= 0) {
        resolve();
        return;
      }
      requestAnimationFrame(() => step(left - 1));
    };
    step(count);
  });
}

/** Presses and keys are ignored while an operation runs (criterion 47a). */
function swallow(event: Event) {
  event.preventDefault();
  event.stopPropagation();
}

const SWALLOWED_EVENTS = [
  "pointerdown",
  "pointerup",
  "click",
  "dblclick",
  "keydown",
  "keyup",
  "wheel",
];

/**
 * The busy state of a running operation: the wait cursor over the whole window and every press,
 * key and wheel turn swallowed in the capture phase until it ends (criterion 47a).
 */
function useBusyWindow(busy: boolean) {
  useEffect(() => {
    if (!busy) {
      return undefined;
    }
    const root = document.documentElement;
    root.dataset.booleanBusy = "true";
    // Not passive, so a wheel turn can be cancelled.
    const options = { capture: true, passive: false };
    for (const name of SWALLOWED_EVENTS) {
      window.addEventListener(name, swallow, options);
    }
    return () => {
      delete root.dataset.booleanBusy;
      for (const name of SWALLOWED_EVENTS) {
        window.removeEventListener(name, swallow, options);
      }
    };
  }, [busy]);
}

/**
 * What the Boolean toolbox of the tool rail shows and does (`specs/0016-boolean-operations/`):
 * the availability the session reports, the busy state around a kernel call, and the notice of
 * the last operation. The decisions (what is enabled, what an operation does, why it was
 * refused) are the session's; this hook only displays them.
 */
export function useBooleanCommands(editor: EditorSession): BooleanCommands {
  const { getSession, syncRevision, tool, selectionCount, applyBoolean } = editor;
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  /** Whether the red outline of a refusal is on the canvas. */
  const outlinedRef = useRef(false);
  useBusyWindow(busy);

  const availability = useMemo<BooleanAvailabilityState>(() => {
    const raw = getSession()?.boolean_availability();
    if (!raw) {
      return { needsTwo: true, open: 0, of: 0 };
    }
    const state = { needsTwo: raw.needs_two, open: raw.open, of: raw.of };
    raw.free();
    return state;
    // The session is read again after every sync (`syncRevision`).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [getSession, syncRevision]);
  const availabilityRef = useRef(availability);
  availabilityRef.current = availability;

  const clearOutline = useCallback(() => {
    if (outlinedRef.current) {
      outlinedRef.current = false;
      getSession()?.clear_boolean_refusal();
    }
  }, [getSession]);
  const { notice, show } = useActionNotice(tool, selectionCount, clearOutline);

  const apply = useCallback(
    (op: BooleanOp) => {
      if (busyRef.current || availabilityRef.current.needsTwo) {
        return;
      }
      busyRef.current = true;
      setBusy(true);
      void (async () => {
        try {
          // Let the busy state reach the screen before the kernel call takes the thread.
          await afterFrames(2);
          const result = applyBoolean(op);
          outlinedRef.current = refusalOutlinesObjects(result);
          const applied = result.kind === "applied";
          const text = applied ? successText(op, result) : refusalText(op, result);
          if (text !== null) {
            show(applied ? "success" : "refusal", text);
          }
        } finally {
          busyRef.current = false;
          setBusy(false);
        }
      })();
    },
    [applyBoolean, show],
  );

  return { availability, busy, notice, apply };
}
