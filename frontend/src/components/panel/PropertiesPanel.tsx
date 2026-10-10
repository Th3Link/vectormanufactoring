import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { DocumentSection } from "@/components/panel/DocumentSection";
import { PanelTabStrip } from "@/components/panel/PanelTabStrip";
import { StyleSection } from "@/components/panel/StyleSection";
import { Tabs, TabsContent } from "@/components/ui/tabs";
import { Tooltip, TooltipProvider } from "@/components/ui/tooltip";
import type { DocumentPanelApi } from "@/hooks/useDocumentPanel";
import type { EditorSession } from "@/hooks/useEditorSession";
import { useBackgroundPanel } from "@/hooks/useBackgroundPanel";
import { useFormats } from "@/hooks/useFormats";
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
 * `Shift+Ctrl+F` (Style, or Document with nothing selected) and `Shift+Ctrl+D`
 * (Document) expand the panel, choose the tab and move focus to its first
 * control; they never collapse it and are ignored during a canvas drag.
 *
 * The header row holds the strip of tabs and the subject line and does not
 * scroll; the body below it does (`specs/0043-properties-tabs/` criteria 18,
 * 19). Which tab is active, and when it switches, is the session's rule.
 */
export function PropertiesPanel({ editor, document: doc }: PropertiesPanelProps) {
  const [open, setOpen] = useState(true);
  const panelId = useId();
  const asideRef = useRef<HTMLElement>(null);
  /** A shortcut asked for the first control: it is focused once the chosen
   * tab's body has rendered (and the panel has opened). */
  const focusAfterShortcut = useRef(false);
  const panel = useStylePanel(editor);
  const background = useBackgroundPanel(editor);
  const { getSession } = editor;
  const formats = useFormats(getSession, doc);
  const { shortcut } = doc;
  const returnFocus = () => editor.containerRef.current?.focus();
  const picking = panel.view.pickTarget !== "" || background.view.picking;
  const endPick = () => {
    panel.endPick();
    background.endPick();
  };

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
      const key = event.key.toLowerCase();
      if (!(event.shiftKey && (event.ctrlKey || event.metaKey)) || (key !== "f" && key !== "d")) {
        return;
      }
      event.preventDefault();
      if (getSession()?.pointer_is_down()) {
        return;
      }
      focusAfterShortcut.current = true;
      shortcut(key === "f" ? "style" : "document");
      if (!open) {
        setOpenKeepingView(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, getSession, shortcut, setOpenKeepingView]);

  useEffect(() => {
    if (open && focusAfterShortcut.current) {
      focusAfterShortcut.current = false;
      focusFirstControl();
    }
  }, [open, doc.view, focusFirstControl]);

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

  const subject =
    doc.view.content === "document"
      ? doc.view.presets.subject
      : doc.view.content === "style"
        ? panel.view.subject
        : "";
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
            if (
              !(event.target as HTMLElement).closest(
                "input, button, [role]:not([role=tabpanel]), select",
              )
            ) {
              event.preventDefault();
              returnFocus();
            }
          }}
          onPointerDownCapture={(event) => {
            // A press anywhere in the panel other than the eyedropper button ends
            // picking and writes nothing (`0017` criterion 26).
            if (picking && !(event.target as HTMLElement).closest("[data-eyedropper]")) {
              endPick();
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
              // Picking ends first and keeps the focus where it is.
              if (picking) {
                endPick();
                return;
              }
              returnFocus();
            }
          }}
          className="properties-panel flex h-full w-[280px] flex-col overflow-hidden border-l border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] bg-[var(--panel-bg)] outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)]"
        >
          <Tabs
            value={doc.view.activeTab}
            onValueChange={doc.pressTab}
            className="flex min-h-0 flex-1 flex-col"
          >
            <div className="flex h-7 shrink-0 items-center justify-between gap-2 px-3 pt-3 pb-2 box-content">
              <PanelTabStrip
                tabs={doc.view.tabs}
                active={doc.view.activeTab}
                onReturnFocus={returnFocus}
              />
              <PanelSubject text={subject} />
            </div>
            {/* Keyed on the tab: the scroll position starts at the top on every
                tab change (criterion 19). */}
            <div
              key={doc.view.activeTab}
              className="properties-panel-body min-h-0 flex-1 px-3 pb-3"
            >
              <TabsContent value={doc.view.activeTab} tabIndex={-1} className="outline-none">
                {doc.view.content === "document" ? (
                  <DocumentSection
                    document={doc}
                    formats={formats}
                    background={background}
                    onReturnFocus={returnFocus}
                  />
                ) : doc.view.content === "style" ? (
                  <StyleSection panel={panel} onReturnFocus={returnFocus} />
                ) : null}
              </TabsContent>
            </div>
          </Tabs>
        </aside>
      </div>
    </TooltipProvider>
  );
}

/** The subject line at the right of the header row: 12 px muted, one line, the
 * full text in a tooltip. */
function PanelSubject({ text }: { text: string }) {
  if (text === "") {
    return null;
  }
  return (
    <Tooltip side="left" content={text}>
      <p className="min-w-0 truncate text-xs text-[var(--panel-muted-fg)]">{text}</p>
    </Tooltip>
  );
}
