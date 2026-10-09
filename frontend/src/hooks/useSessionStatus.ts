import { useEffect, useState } from "react";

import type { EditorSession } from "@/hooks/useEditorSession";

/** What the status bar and the ruler corner read from the live session. */
export interface SessionStatus {
  /** The document size with the display unit, e.g. "210.0 × 297.0 mm". */
  sizeText: string;
  /** The display unit's symbol: "mm", "cm" or "in". */
  unit: string;
  /** The cursor readout for a document point in millimetres, e.g.
   * "x: 12.3  y: 45.6 mm". */
  cursorText: (xMm: number, yMm: number) => string;
}

const INITIAL: SessionStatus = {
  sizeText: "210.0 × 297.0 mm",
  unit: "mm",
  cursorText: (x, y) => `x: ${x.toFixed(1)}  y: ${y.toFixed(1)} mm`,
};

/**
 * Reads the document size text and the display unit from the live session
 * every animation frame and re-renders only when one of them changed, so a
 * New, an Open, a resize or a unit change shows at once without any of them
 * having to notify the host (`specs/0015-document-size-and-rulers/` criteria
 * 12 and 21: the size comes from the session, not from the Tauri host).
 */
export function useSessionStatus(editor: EditorSession): SessionStatus {
  const { getSession } = editor;
  const [state, setState] = useState({
    sizeText: INITIAL.sizeText,
    unit: INITIAL.unit,
  });

  useEffect(() => {
    let frame = requestAnimationFrame(function poll() {
      frame = requestAnimationFrame(poll);
      const session = getSession();
      if (!session) {
        return;
      }
      const sizeText = session.size_text();
      const unit = session.display_unit();
      setState((previous) =>
        previous.sizeText === sizeText && previous.unit === unit
          ? previous
          : { sizeText, unit },
      );
    });
    return () => cancelAnimationFrame(frame);
  }, [getSession]);

  return {
    ...state,
    cursorText: (xMm, yMm) =>
      getSession()?.cursor_text(xMm, yMm) ?? INITIAL.cursorText(xMm, yMm),
  };
}
