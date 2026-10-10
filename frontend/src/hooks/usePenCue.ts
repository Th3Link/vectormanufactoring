import { useCallback, useEffect, useMemo, useState } from "react";

import type { EditorSession } from "@/hooks/useEditorSession";
import { NO_PEN_CUE, type ClosePathState, type PenCue } from "@/lib/penText";

function sameCue(a: PenCue, b: PenCue): boolean {
  return (
    a.kind === b.kind &&
    a.target === b.target &&
    a.join === b.join &&
    a.asDrawn === b.asDrawn &&
    a.shift === b.shift &&
    a.styleDiffers === b.styleDiffers
  );
}

/**
 * What a Pen press at the pointer would do (`specs/0034-pen-path-extension/`): the session's
 * `pen_cue`, read again after every pointer event and every key (Shift changes it with the pointer
 * at rest) and after every sync. Only in the Pen tool; the decisions are Rust's. The read waits a
 * microtask so the editor's own handler has already told the session about the event.
 */
export function usePenCue(editor: EditorSession): PenCue {
  const { getSession, syncRevision, tool, containerRef } = editor;
  const [cue, setCue] = useState<PenCue>(NO_PEN_CUE);

  const refresh = useCallback(() => {
    let next = NO_PEN_CUE;
    const raw = tool === "pen" ? getSession()?.pen_cue() : undefined;
    if (raw) {
      next = {
        kind: raw.kind,
        target: raw.target,
        join: raw.join,
        asDrawn: raw.as_drawn,
        shift: raw.shift,
        styleDiffers: raw.style_differs,
      };
      raw.free();
    }
    setCue((held) => (sameCue(held, next) ? held : next));
  }, [getSession, tool]);

  useEffect(() => {
    refresh();
  }, [refresh, syncRevision]);

  useEffect(() => {
    if (tool !== "pen") {
      return undefined;
    }
    const container = containerRef.current;
    const later = () => queueMicrotask(refresh);
    const pointerEvents = ["pointermove", "pointerdown", "pointerup", "pointerleave"];
    for (const name of pointerEvents) {
      container?.addEventListener(name, later);
    }
    window.addEventListener("keydown", later);
    window.addEventListener("keyup", later);
    return () => {
      for (const name of pointerEvents) {
        container?.removeEventListener(name, later);
      }
      window.removeEventListener("keydown", later);
      window.removeEventListener("keyup", later);
    };
  }, [tool, containerRef, refresh]);

  return tool === "pen" ? cue : NO_PEN_CUE;
}

const NO_CLOSE_STATE: ClosePathState = { closable: 0, skipped: 0, sameEnds: 0 };

/** What the Node bar's Close path buttons would do now: how many open paths they close and skip. */
export function useClosePathState(editor: EditorSession): ClosePathState {
  const { getSession, syncRevision, tool } = editor;
  return useMemo(() => {
    if (tool !== "node") {
      return NO_CLOSE_STATE;
    }
    const raw = getSession()?.close_path_state();
    if (!raw) {
      return NO_CLOSE_STATE;
    }
    const state = {
      closable: raw.closable,
      skipped: raw.skipped,
      sameEnds: raw.same_ends,
    };
    raw.free();
    return state;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [getSession, syncRevision, tool]);
}
