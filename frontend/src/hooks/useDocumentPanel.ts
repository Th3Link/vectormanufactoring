import { useCallback, useMemo, useState } from "react";

import type { WasmSession } from "@/lib/editorSession";

/** What the Properties panel shows (`curvyo-ui-core::panel_content`). */
export type PanelContent = "document" | "style" | "empty";

/** The unit symbols the document stores (`DisplayUnit`). */
export type UnitSymbol = "mm" | "cm" | "in";

/** What Fit to content did. */
export type FitResult =
  | { kind: "fitted" }
  | { kind: "already-fits" }
  | { kind: "empty" }
  | { kind: "too-large"; message: string };

/** A plain-JS copy of what the session says about the document: the panel's
 * content, the size fields, the unit and the status bar's size text. Every
 * text is formatted in Rust. */
export interface DocumentView {
  content: PanelContent;
  unit: UnitSymbol;
  widthText: string;
  heightText: string;
  /** The message of a refused size, the limits in the display unit. */
  sideMessage: string;
  hasObjects: boolean;
  /** The status bar's size readout, e.g. "210.0 × 297.0 mm". */
  sizeText: string;
}

const INITIAL: DocumentView = {
  content: "document",
  unit: "mm",
  widthText: "210",
  heightText: "297",
  sideMessage: "Enter a number from 1 to 100000",
  hasObjects: false,
  sizeText: "210.0 × 297.0 mm",
};

function readView(session: WasmSession | null): DocumentView {
  if (!session) {
    return INITIAL;
  }
  return {
    content: session.panel_content() as PanelContent,
    unit: session.display_unit() as UnitSymbol,
    widthText: session.document_side_text("width"),
    heightText: session.document_side_text("height"),
    sideMessage: session.document_side_message(),
    hasObjects: session.document_has_objects(),
    sizeText: session.size_text(),
  };
}

export interface DocumentPanelApi {
  view: DocumentView;
  /** The status bar's cursor readout for a document point in millimetres. */
  cursorText: (xMm: number, yMm: number) => string;
  /** Enter, Tab or leaving a size field: `"committed"`, `"unchanged"` or
   * `"invalid:number"`. */
  setSide: (side: "width" | "height", text: string) => string;
  setUnit: (unit: UnitSymbol) => void;
  fit: () => FitResult;
}

interface EditorHandle {
  getSession: () => WasmSession | null;
  syncRevision: number;
}

/**
 * The document's panel state and commands (`specs/0015-document-size-and-
 * rulers/`, criteria 12, 14 to 27a, 33 to 36). The snapshot is re-read
 * whenever the session says its UI state changed (`syncRevision`: a New, an
 * Open, a selection, a pen click) or a command here changed something. Every
 * rule is the session's; this forwards each control's command.
 */
export function useDocumentPanel(editor: EditorHandle): DocumentPanelApi {
  const { getSession, syncRevision } = editor;
  const [local, setLocal] = useState(0);
  const refresh = useCallback(() => setLocal((n) => n + 1), []);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const view = useMemo(() => readView(getSession()), [getSession, syncRevision, local]);

  const setSide = useCallback(
    (side: "width" | "height", text: string): string => {
      const session = getSession();
      if (!session) {
        return "unchanged";
      }
      const outcome = session.set_document_side(side, text);
      refresh();
      return outcome;
    },
    [getSession, refresh],
  );

  const setUnit = useCallback(
    (unit: UnitSymbol) => {
      getSession()?.set_display_unit(unit);
      refresh();
    },
    [getSession, refresh],
  );

  const fit = useCallback((): FitResult => {
    const session = getSession();
    if (!session) {
      return { kind: "empty" };
    }
    const outcome = session.fit_document();
    refresh();
    if (outcome.startsWith("too-large:")) {
      return { kind: "too-large", message: outcome.slice("too-large:".length) };
    }
    return { kind: outcome as "fitted" | "already-fits" | "empty" };
  }, [getSession, refresh]);

  const cursorText = useCallback(
    (xMm: number, yMm: number) =>
      getSession()?.cursor_text(xMm, yMm) ?? `x: ${xMm.toFixed(1)}  y: ${yMm.toFixed(1)} mm`,
    [getSession],
  );

  return { view, cursorText, setSide, setUnit, fit };
}
