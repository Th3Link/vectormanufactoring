import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { DocumentSection } from "@/components/panel/DocumentSection";
import { StyleSection } from "@/components/panel/StyleSection";
import { Tooltip, TooltipProvider } from "@/components/ui/tooltip";
import type { DocumentPanelApi } from "@/hooks/useDocumentPanel";
import type { EditorSession } from "@/hooks/useEditorSession";
import { useStylePanel } from "@/hooks/useStylePanel";

/** The panel's width, px (`docs/design-system.md`, "Properties panel"). */
export const PANEL_WIDTH_PX = 280;

interface PropertiesPanelProps {
  editor: EditorSession;
  document: DocumentPanelApi;
}

/**
 * The right-docked properties panel (`specs/0007-stroke-and-fill-styling`
 * criteria 38, 39): 280 px, full height down to the status bar, a sibling of
 * the canvas region and never an overlay. A 16 x 48 px tab on its canvas-facing
 * edge collapses it (state per session, default open). Collapsed, the content
 * is hidden and out of the tab order and the canvas region takes the width; the document does not move
 * on screen because the resize keeps the view's top-left origin.
 * `Shift+Ctrl+F` expands the panel and moves focus to its first control; it
 * never collapses it and is ignored during a canvas drag.
 */
export function PropertiesPanel({ editor, document: doc }: PropertiesPanelProps) {
  const [open, setOpen] = useState(true);
  const panelId = useId();
  const asideRef = useRef<HTMLElement>(null);
  const focusAfterOpen = useRef(false);
  const panel = useStylePanel(editor);
  const { getSession } = editor;
  const returnFocus = () => editor.containerRef.current?.focus();

  const setOpenKeepingView = useCallback(
    (next: boolean) => {
      // The canvas is about to gain or lose the panel's width: keep the
      // document where it is on screen (criterion 39).
      getSession()?.keep_view_origin_for_panel_toggle(next ? -PANEL_WIDTH_PX : PANEL_WIDTH_PX);
      setOpen(next);
    },
    [getSession],
  );

  /** Focus lands on the first control (Stroke Paint), or on the panel itself
   * when it is empty. */
  const focusFirstControl = useCallback(() => {
    const aside = asideRef.current;
    if (!aside) {
      return;
    }
    const first =
      aside.querySelector<HTMLElement>("[data-first-focus]") ??
      aside.querySelector<HTMLElement>("input, button");
    const target = first ?? aside;
    // A control focused by the shortcut shows the focus ring even if the
    // webview does not count script focus as keyboard focus; the mark goes
    // when the focus does.
    target.focus({ focusVisible: true } as FocusOptions);
    target.setAttribute("data-keyboard-focus", "");
    target.addEventListener("blur", () => target.removeAttribute("data-keyboard-focus"), {
      once: true,
    });
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        !(event.shiftKey && (event.ctrlKey || event.metaKey)) ||
        event.key.toLowerCase() !== "f"
      ) {
        return;
      }
      event.preventDefault();
      if (getSession()?.pointer_is_down()) {
        return;
      }
      if (open) {
        focusFirstControl();
      } else {
        focusAfterOpen.current = true;
        setOpenKeepingView(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, getSession, focusFirstControl, setOpenKeepingView]);

  useEffect(() => {
    if (open && focusAfterOpen.current) {
      focusAfterOpen.current = false;
      focusFirstControl();
    }
  }, [open, focusFirstControl]);

  // A control that had keyboard focus can leave the tree when the selection or
  // the tool changes (the panel becomes empty or shows the Document section):
  // focus goes to the canvas, not to the page body (`0017` criterion 1).
  const focusInside = useRef(false);
  const content = doc.view.content;
  useEffect(() => {
    const aside = asideRef.current;
    if (focusInside.current && aside && !aside.contains(window.document.activeElement)) {
      focusInside.current = false;
      returnFocus();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [content, panel.view.scopeKey]);

  const label = open ? "Hide properties panel" : "Show properties panel";
  return (
    <TooltipProvider>
      <div
        className="relative shrink-0"
        style={{ width: open ? PANEL_WIDTH_PX : 0 }}
      >
        <Tooltip side="left" content="Properties (Shift+Ctrl+F)">
          <button
            type="button"
            aria-label={label}
            aria-expanded={open}
            aria-controls={panelId}
            onClick={(event) => {
              setOpenKeepingView(!open);
              // `detail` is 0 for keyboard activation, 1 or more for a click.
              if (event.detail > 0) {
                returnFocus();
              }
            }}
            // The hit target is 24 px wide (WCAG 2.2 AA 2.5.8); the 16 px tab
            // of the design system is drawn inside it, on the canvas side.
            className="group absolute top-1/2 left-[-24px] z-30 flex h-12 w-6 -translate-y-1/2 items-center justify-end outline-none"
          >
            <span
              className="flex h-12 w-4 items-center justify-center rounded-l-md border border-r-0 border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] bg-[var(--toolbar-bg)] text-[var(--toolbar-icon)] group-hover:bg-[var(--editor-accent-hover)] group-focus-visible:ring-2 group-focus-visible:ring-[var(--editor-accent)]"
              style={{ boxShadow: "var(--panel-elevation-shadow)" }}
            >
              {open ? <ChevronRight size={12} aria-hidden /> : <ChevronLeft size={12} aria-hidden />}
            </span>
          </button>
        </Tooltip>
        <aside
          id={panelId}
          ref={asideRef}
          aria-label="Properties"
          tabIndex={-1}
          hidden={!open}
          onFocus={() => {
            focusInside.current = true;
          }}
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              focusInside.current = false;
            }
          }}
          onMouseDown={(event) => {
            // A press on dead space (the heading, a label) must not leave the
            // focus on the panel, where the tool letters would do nothing: it
            // goes back to the canvas. A control keeps the press.
            if (!(event.target as HTMLElement).closest("input, button, [role], select")) {
              event.preventDefault();
              returnFocus();
            }
          }}
          onKeyDown={(event) => {
            // Escape in the panel (`0017` criterion 60): a running drag is
            // reverted first (`usePreviewGesture` takes the key before it gets
            // here); a field restores itself and returns focus; otherwise focus
            // returns to the canvas. It never clears the selection and never
            // reaches the canvas.
            if (event.key === "Escape" && event.currentTarget.contains(event.target as Node)) {
              event.preventDefault();
              returnFocus();
            }
          }}
          className="properties-panel h-full w-[280px] border-l border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] bg-[var(--panel-bg)] p-3 outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)]"
        >
          {doc.view.content === "document" ? (
            <DocumentSection document={doc} onReturnFocus={returnFocus} />
          ) : doc.view.content === "style" ? (
            <StyleSection panel={panel} onReturnFocus={returnFocus} />
          ) : null}
        </aside>
      </div>
    </TooltipProvider>
  );
}
