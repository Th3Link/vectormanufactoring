import { CircleAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { DocumentPresets } from "@/components/panel/DocumentPresets";
import { EntryField } from "@/components/panel/EntryField";
import { StyleRow } from "@/components/panel/StyleRow";
import { ToggleGroup, type ToggleOption } from "@/components/ui/toggle-group";
import { Tooltip } from "@/components/ui/tooltip";
import type { DocumentPanelApi, UnitSymbol } from "@/hooks/useDocumentPanel";

const UNIT_TOOLTIP =
  "How lengths are shown on the rulers, here and in the status bar. Other fields stay in mm. Stored sizes are always mm.";

const UNIT_OPTIONS: readonly ToggleOption<UnitSymbol>[] = (
  [
    // The accessible name starts with the visible text (WCAG 2.5.3).
    ["mm", "mm, millimetres"],
    ["cm", "cm, centimetres"],
    ["in", "in, inches"],
  ] as const
).map(([value, label]) => ({
  value,
  label,
  tooltip: UNIT_TOOLTIP,
  icon: <span className="text-sm">{value}</span>,
}));

const SIZE_TOOLTIP =
  "Document size. A resize keeps the center, so objects move with the document and nothing changes on screen.";
const FIT_TOOLTIP =
  "Resize the document to the extent of all objects, without margin. Objects move so the extent starts at 0, 0.";

/** How long "Already fits the content." stays, ms. */
const NOTICE_MS = 3000;
/** How long the refusal of a too-large fit stays, ms. */
const REFUSAL_MS = 8000;

interface DocumentSectionProps {
  document: DocumentPanelApi;
  onReturnFocus: () => void;
}

/**
 * The "Document" section of the properties panel (`specs/0015-document-size-
 * and-rulers`, `docs/design-system.md`, "Properties panel: Document section"):
 * Width and Height typed fields (a resize keeps the center), the display unit,
 * and Fit to content. Every value and every rule comes from the session; this
 * holds the fields' text and the two short messages under the button.
 */
export function DocumentSection({ document: doc, onReturnFocus }: DocumentSectionProps) {
  const { view } = doc;
  const messages = { number: view.sideMessage } as const;
  const [notice, setNotice] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);

  // A refusal goes at the next press anywhere, or after a while.
  useEffect(() => {
    if (refusal === null) {
      return;
    }
    const clear = () => setRefusal(null);
    const handle = window.setTimeout(clear, REFUSAL_MS);
    window.addEventListener("pointerdown", clear, { once: true });
    window.addEventListener("keydown", clear, { once: true });
    return () => {
      window.clearTimeout(handle);
      window.removeEventListener("pointerdown", clear);
      window.removeEventListener("keydown", clear);
    };
  }, [refusal]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const fit = () => {
    const result = doc.fit();
    window.clearTimeout(timer.current);
    setNotice(null);
    setRefusal(null);
    if (result.kind === "already-fits") {
      setNotice("Already fits the content.");
      timer.current = window.setTimeout(() => setNotice(null), NOTICE_MS);
    } else if (result.kind === "too-large") {
      setRefusal(result.message);
    }
  };

  return (
    <section aria-labelledby="document-heading" className="flex flex-col gap-2">
      <div className="flex h-6 items-center justify-between gap-2">
        <h2 id="document-heading" className="text-sm font-semibold text-[var(--toolbar-icon)]">
          Document
        </h2>
        <p className="truncate text-xs text-[var(--panel-muted-fg)]">{view.presets.subject}</p>
      </div>
      <DocumentPresets document={doc} onReturnFocus={onReturnFocus} />
      <div
        aria-hidden
        className="my-1 h-px"
        style={{ background: "color-mix(in srgb, var(--toolbar-icon) 25%, transparent)" }}
      />
      <StyleRow label="Width">
        <Tooltip side="left" content={SIZE_TOOLTIP}>
          <div>
            <EntryField
              label="Document width"
              shown={view.widthText}
              mixed={false}
              suffix={view.unit}
              width={176}
              commitOnBlur
              inputMode="decimal"
              onSubmit={(text) => doc.setSide("width", text)}
              messages={messages}
              onReturnFocus={onReturnFocus}
              firstFocus
            />
          </div>
        </Tooltip>
      </StyleRow>
      <StyleRow label="Height">
        <Tooltip side="left" content={SIZE_TOOLTIP}>
          <div>
            <EntryField
              label="Document height"
              shown={view.heightText}
              mixed={false}
              suffix={view.unit}
              width={176}
              commitOnBlur
              inputMode="decimal"
              onSubmit={(text) => doc.setSide("height", text)}
              messages={messages}
              onReturnFocus={onReturnFocus}
            />
          </div>
        </Tooltip>
      </StyleRow>
      <StyleRow label="Unit">
        <ToggleGroup
          label="Display unit"
          options={UNIT_OPTIONS}
          value={view.unit}
          onChange={doc.setUnit}
          itemWidth={44}
          onReturnFocus={onReturnFocus}
        />
      </StyleRow>
      {view.hasObjects && (
        <div className="relative">
          <Tooltip side="left" content={FIT_TOOLTIP}>
            <button
              type="button"
              onClick={(event) => {
                fit();
                // `detail` is 0 for keyboard activation, 1 or more for a click.
                if (event.detail > 0) {
                  onReturnFocus();
                }
              }}
              className="h-7 w-full rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] bg-transparent text-sm text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)]"
            >
              Fit to content
            </button>
          </Tooltip>
          {/* The live region is always in the tree, visually hidden, so its text
              change is announced; the visible notice is a text-only copy. */}
          <p role="status" className="sr-only">
            {notice}
          </p>
          {notice && (
            <p
              aria-hidden
              className="pointer-events-none absolute top-full right-0 z-30 mt-1 max-w-[244px] rounded-md bg-[var(--toolbar-bg)] px-2 py-1 text-xs text-[var(--toolbar-icon)]"
              style={{ boxShadow: "var(--panel-elevation-shadow)" }}
            >
              {notice}
            </p>
          )}
          {refusal && (
            <div
              role="alert"
              className="pointer-events-none absolute top-full right-0 z-30 mt-1 flex max-w-[244px] items-start gap-1 rounded-md bg-popover px-2 py-1 text-xs text-[var(--field-invalid)] ring-1 ring-[var(--field-invalid)]"
            >
              <CircleAlert size={12} aria-hidden className="mt-0.5 shrink-0" />
              {refusal}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
