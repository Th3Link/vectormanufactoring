import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { EditorSession } from "@/hooks/useEditorSession";
import {
  type BooleanAvailabilityState,
  type BooleanOp,
  refusalOutlinesObjects,
  refusalText,
  successText,
} from "@/lib/booleanText";

/** How long a success notice stays (criterion 29). */
const SUCCESS_NOTICE_MS = 3000;
/** How long a refusal notice stays (criterion 15). */
const REFUSAL_NOTICE_MS = 8000;

/** The notice beside the rail: a success is a polite status, a refusal an alert. */
export interface BooleanNotice {
  /** Changes with every notice, so the 3 s or 8 s life restarts. */
  id: number;
  kind: "success" | "refusal";
  text: string;
}

export interface BooleanCommands {
  availability: BooleanAvailabilityState;
  /** A kernel call is about to run or running: wait cursor, input ignored. */
  busy: boolean;
  notice: BooleanNotice | null;
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

/** Keys and presses are ignored while an operation runs (criterion 47a). */
function swallow(event: Event) {
  event.preventDefault();
  event.stopPropagation();
}

/**
 * The state of the Boolean section of the tool rail (`specs/0016-boolean-operations/`):
 * what the buttons show, the busy state around a kernel call, and the notice
 * beside the rail. The decisions (what is enabled, what an operation does, why
 * it was refused) are the session's; this hook only displays them.
 */
export function useBooleanCommands(editor: EditorSession): BooleanCommands {
  const { getSession, syncRevision, tool, selectionCount, applyBoolean } = editor;
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [notice, setNotice] = useState<BooleanNotice | null>(null);
  const noticeCounter = useRef(0);
  /** Whether the red outline of a refusal is on the canvas. */
  const outlinedRef = useRef(false);
  /** The tool and selection size the notice was posted with. */
  const baselineRef = useRef({ tool, selectionCount });

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

  const dismiss = useCallback(() => {
    setNotice(null);
    if (outlinedRef.current) {
      outlinedRef.current = false;
      getSession()?.clear_boolean_refusal();
    }
  }, [getSession]);

  // The next action ends a notice: a press, a key, a selection change, a tool change.
  useEffect(() => {
    baselineRef.current = { tool, selectionCount };
  }, [notice?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (
      notice &&
      (baselineRef.current.tool !== tool ||
        baselineRef.current.selectionCount !== selectionCount)
    ) {
      dismiss();
    }
  }, [tool, selectionCount, notice, dismiss]);
  useEffect(() => {
    if (!notice) {
      return undefined;
    }
    const timer = window.setTimeout(
      dismiss,
      notice.kind === "success" ? SUCCESS_NOTICE_MS : REFUSAL_NOTICE_MS,
    );
    window.addEventListener("pointerdown", dismiss, true);
    window.addEventListener("keydown", dismiss, true);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("pointerdown", dismiss, true);
      window.removeEventListener("keydown", dismiss, true);
    };
  }, [notice, dismiss]);

  // While an operation runs: wait cursor everywhere, no press or key gets through.
  useEffect(() => {
    if (!busy) {
      return undefined;
    }
    const root = document.documentElement;
    root.dataset.booleanBusy = "true";
    const events = ["pointerdown", "pointerup", "click", "dblclick", "keydown", "keyup", "wheel"];
    for (const name of events) {
      window.addEventListener(name, swallow, true);
    }
    return () => {
      delete root.dataset.booleanBusy;
      for (const name of events) {
        window.removeEventListener(name, swallow, true);
      }
    };
  }, [busy]);

  const apply = useCallback(
    (op: BooleanOp) => {
      if (busyRef.current || availabilityRef.current.needsTwo) {
        return;
      }
      busyRef.current = true;
      setBusy(true);
      void (async () => {
        try {
          // Let the busy state reach the screen before the kernel call
          // takes the thread (criterion 47a).
          await afterFrames(2);
          const result = applyBoolean(op);
          const text =
            result.kind === "applied" ? successText(op, result) : refusalText(op, result);
          noticeCounter.current += 1;
          outlinedRef.current = refusalOutlinesObjects(result);
          setNotice(
            text === null
              ? null
              : {
                  id: noticeCounter.current,
                  kind: result.kind === "applied" ? "success" : "refusal",
                  text,
                },
          );
        } finally {
          busyRef.current = false;
          setBusy(false);
        }
      })();
    },
    [applyBoolean],
  );

  return { availability, busy, notice, apply };
}
