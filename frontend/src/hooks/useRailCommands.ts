import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { type ActionNotice, useActionNotice } from "@/hooks/useActionNotice";
import type { EditorSession } from "@/hooks/useEditorSession";
import {
  breakApartRefusalText,
  breakApartSuccessText,
  combineOutlinesObjects,
  combineRefusalText,
  combineSuccessText,
  commandDimmed,
  noticeMs,
  type PathAvailabilityState,
  type PathCommand,
} from "@/lib/pathText";
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
  /** Runs `op` on the selection. Does nothing while dimmed or busy. */
  apply: (op: BooleanOp) => void;
}

/** What the Path card shows and does (`specs/0035-combine-and-break-apart/`). */
export interface PathCommands {
  availability: PathAvailabilityState;
  /** A command is about to run or running: wait cursor, input ignored. */
  busy: boolean;
  /** Runs `command` on the selection. Does nothing while dimmed or busy. */
  apply: (command: PathCommand) => void;
}

/** Everything the tool rail's command cards need: both cards and the one notice they share (one
 * status region and one alert region for the whole rail). */
export interface RailCommands {
  booleans: BooleanCommands;
  path: PathCommands;
  notice: ActionNotice | null;
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

const NO_PATH_AVAILABILITY: PathAvailabilityState = {
  needsTwo: true,
  open: 0,
  of: 0,
  canBreakApart: false,
};

/**
 * What the command cards of the tool rail show and do (`specs/0016-boolean-operations/`,
 * `specs/0035-combine-and-break-apart/`): the availability the session reports, the busy state
 * around a call, and the notice of the last command. The decisions (what is enabled, what a
 * command does, why it was refused) are the session's; this hook only displays them.
 */
export function useRailCommands(editor: EditorSession): RailCommands {
  const { getSession, syncRevision, tool, selectionCount, applyBoolean } = editor;
  const { applyCombine, applyBreakApart } = editor;
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  /** Whether the red outline of a refusal is on the canvas. */
  const outlinedRef = useRef(false);
  useBusyWindow(busy);

  const booleanAvailability = useMemo<BooleanAvailabilityState>(() => {
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
  const pathAvailability = useMemo<PathAvailabilityState>(() => {
    const raw = getSession()?.path_availability();
    if (!raw) {
      return NO_PATH_AVAILABILITY;
    }
    const state = {
      needsTwo: raw.needs_two,
      open: raw.open,
      of: raw.of,
      canBreakApart: raw.can_break_apart,
    };
    raw.free();
    return state;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [getSession, syncRevision]);
  const booleanRef = useRef(booleanAvailability);
  booleanRef.current = booleanAvailability;
  const pathRef = useRef(pathAvailability);
  pathRef.current = pathAvailability;

  const clearOutline = useCallback(() => {
    if (outlinedRef.current) {
      outlinedRef.current = false;
      getSession()?.clear_boolean_refusal();
    }
  }, [getSession]);
  const { notice, show } = useActionNotice(tool, selectionCount, clearOutline);

  /** Runs `run` after the busy state has reached the screen, then shows its notice. */
  const runBusy = useCallback((run: () => void) => {
    busyRef.current = true;
    setBusy(true);
    void (async () => {
      try {
        // Let the busy state reach the screen before the call takes the thread.
        await afterFrames(2);
        run();
      } finally {
        busyRef.current = false;
        setBusy(false);
      }
    })();
  }, []);

  const apply = useCallback(
    (op: BooleanOp) => {
      if (busyRef.current || booleanRef.current.needsTwo) {
        return;
      }
      runBusy(() => {
        const result = applyBoolean(op);
        outlinedRef.current = refusalOutlinesObjects(result);
        const applied = result.kind === "applied";
        const text = applied ? successText(op, result) : refusalText(op, result);
        if (text !== null) {
          show(applied ? "success" : "refusal", text);
        }
      });
    },
    [applyBoolean, runBusy, show],
  );

  const applyPath = useCallback(
    (command: PathCommand) => {
      if (busyRef.current || commandDimmed(command, pathRef.current)) {
        return;
      }
      runBusy(() => {
        if (command === "combine") {
          const result = applyCombine();
          outlinedRef.current = combineOutlinesObjects(result);
          const success = combineSuccessText(result);
          const refusal = combineRefusalText(result);
          if (success !== null) {
            show("success", success, noticeMs(success), "path");
          } else if (refusal !== null) {
            show("refusal", refusal, undefined, "path");
          }
        } else {
          const result = applyBreakApart();
          outlinedRef.current = false;
          const success = breakApartSuccessText(result);
          const refusal = breakApartRefusalText(result);
          if (success !== null) {
            show("success", success, noticeMs(success), "path");
          } else if (refusal !== null) {
            show("refusal", refusal, undefined, "path");
          }
        }
      });
    },
    [applyBreakApart, applyCombine, runBusy, show],
  );

  return {
    booleans: { availability: booleanAvailability, busy, apply },
    path: { availability: pathAvailability, busy, apply: applyPath },
    notice,
  };
}
