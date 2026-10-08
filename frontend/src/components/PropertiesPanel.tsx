import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { StyleSection } from "@/components/StyleSection";
import { Tooltip, TooltipProvider } from "@/components/ui/tooltip";
import type { EditorSession } from "@/hooks/useEditorSession";
import { useStylePanel } from "@/hooks/useStylePanel";

/** The panel's width, px (`docs/design-system.md`, "Properties panel"). */
export const PANEL_WIDTH_PX = 280;

interface PropertiesPanelProps {
  editor: EditorSession;
}

/**
 * The right-docked properties panel (`specs/0007-stroke-and-fill-styling`
 * criteria 38, 39): 280 px, full height down to the status bar, a sibling of
 * the canvas region and never an overlay. A 16 x 48 px tab on its canvas-facing
 * edge collapses it (state per session, default open). Collapsed, the content
 * is hidden and out of the tab order and the canvas region takes the width; the document does not move
 * on screen because the resize keeps the view's top-left origin.
 * `Shift+Ctrl+F` expands the panel and moves focus to its first enabled
 * control; it never collapses it and is ignored during a canvas drag.
 */
export function PropertiesPanel({ editor }: PropertiesPanelProps) {
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

  /** Focus lands on the first enabled control, or on the panel itself when
   * every control is disabled (it reads the subject line). */
  const focusFirstControl = useCallback(() => {
    const aside = asideRef.current;
    if (!aside) {
      return;
    }
    const first =
      aside.querySelector<HTMLElement>("[data-first-focus]:not(:disabled)") ??
      aside.querySelector<HTMLElement>("input:not(:disabled), button:not(:disabled)");
    (first ?? aside).focus();
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
            className="absolute top-1/2 left-[-16px] z-30 flex h-12 w-4 -translate-y-1/2 items-center justify-center rounded-l-md border border-r-0 border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] bg-[var(--toolbar-bg)] text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)]"
            style={{ boxShadow: "var(--panel-elevation-shadow)" }}
          >
            {open ? <ChevronRight size={12} aria-hidden /> : <ChevronLeft size={12} aria-hidden />}
          </button>
        </Tooltip>
        <aside
          id={panelId}
          ref={asideRef}
          aria-label="Properties"
          tabIndex={-1}
          hidden={!open}
          onKeyDown={(event) => {
            // Escape in the panel: an open popover or select closes first (they
            // handle it in their own portal, outside this element); a field
            // restores itself; otherwise focus just returns to the canvas. It
            // never clears the selection and never reaches the canvas.
            if (
              event.key === "Escape" &&
              !event.defaultPrevented &&
              event.currentTarget.contains(event.target as Node)
            ) {
              event.preventDefault();
              returnFocus();
            }
          }}
          className="properties-panel h-full w-[280px] border-l border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] bg-[var(--panel-bg)] p-3 outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)]"
        >
          <StyleSection
            panel={panel}
            closeKey={`${panel.view.scopeKey}|${editor.tool}`}
            onReturnFocus={returnFocus}
          />
        </aside>
      </div>
    </TooltipProvider>
  );
}
