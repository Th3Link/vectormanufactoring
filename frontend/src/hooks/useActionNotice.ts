import { useCallback, useEffect, useRef, useState } from "react";

/** How long a success notice stays (`specs/0016-boolean-operations/` criterion 29). */
const SUCCESS_NOTICE_MS = 3000;
/** How long a refusal notice stays (criterion 15). */
const REFUSAL_NOTICE_MS = 8000;

/** An action notice (`docs/design-system.md`, row "Action notice"): a success is a polite
 * status, a refusal an alert. */
export interface ActionNotice {
  /** Changes with every notice, so the 3 s or 8 s life restarts. */
  id: number;
  kind: "success" | "refusal";
  text: string;
  /** How long it stays, ms; the default of its kind if absent. */
  ms?: number;
}

export interface ActionNoticeState {
  notice: ActionNotice | null;
  /** Shows a notice in place of the one on screen, for `ms` or the default of its kind. */
  show: (kind: ActionNotice["kind"], text: string, ms?: number) => void;
}

/**
 * The life of one action notice: it ends after 3 s (success) or 8 s (refusal), or at the next
 * action: a press, a key, a selection change or a tool change (not a pointer move). `onEnd`
 * runs when it ends (to remove what the notice came with, such as the red outline of a refusal).
 * A change that the action causes itself, such as the selection shrinking to the result, does
 * not end it: the tool and selection size it was shown with are the baseline.
 */
export function useActionNotice(
  tool: string,
  selectionCount: number,
  onEnd: () => void,
): ActionNoticeState {
  const [notice, setNotice] = useState<ActionNotice | null>(null);
  const counter = useRef(0);
  const baseline = useRef({ tool, selectionCount });

  const end = useCallback(() => {
    setNotice(null);
    onEnd();
  }, [onEnd]);

  const show = useCallback((kind: ActionNotice["kind"], text: string, ms?: number) => {
    counter.current += 1;
    setNotice({ id: counter.current, kind, text, ms });
  }, []);

  // The baseline is taken in the commit that shows the notice, after the state the action
  // changed (tool, selection size) has settled in the same batch.
  useEffect(() => {
    baseline.current = { tool, selectionCount };
  }, [notice?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (
      notice &&
      (baseline.current.tool !== tool || baseline.current.selectionCount !== selectionCount)
    ) {
      end();
    }
  }, [tool, selectionCount, notice, end]);

  useEffect(() => {
    if (!notice) {
      return undefined;
    }
    const timer = window.setTimeout(
      end,
      notice.ms ?? (notice.kind === "success" ? SUCCESS_NOTICE_MS : REFUSAL_NOTICE_MS),
    );
    window.addEventListener("pointerdown", end, true);
    window.addEventListener("keydown", end, true);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("pointerdown", end, true);
      window.removeEventListener("keydown", end, true);
    };
  }, [notice, end]);

  return { notice, show };
}
