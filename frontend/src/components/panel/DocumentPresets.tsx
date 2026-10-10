import { useId } from "react";

import { PresetStrip, type StripCell } from "@/components/panel/PresetStrip";
import type { DocumentPanelApi } from "@/hooks/useDocumentPanel";

/** A tooltip of one or two lines: the second is muted. */
function tooltipOf(text: string) {
  const [first, ...rest] = text.split("\n");
  return (
    <>
      <div>{first}</div>
      {rest.length > 0 && <div className="text-muted-foreground">{rest.join(" ")}</div>}
    </>
  );
}

/** A page glyph: 9 x 12 portrait or 12 x 9 landscape, 16 px, 1.5 px stroke. */
function PageGlyph({ landscape }: { landscape: boolean }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-hidden
    >
      {landscape ? (
        <rect x="2" y="3.5" width="12" height="9" rx="1.5" />
      ) : (
        <rect x="3.5" y="2" width="9" height="12" rx="1.5" />
      )}
    </svg>
  );
}

interface DocumentPresetsProps {
  document: DocumentPanelApi;
  onReturnFocus: () => void;
}

/**
 * The presets of the Document section (`specs/0030-document-size-presets`
 * criteria 9, 14, 16, 17): for each group of the data file a heading and a
 * strip of buttons, then the Portrait / Landscape group. Everything shown
 * (names, tooltips, the pressed button) comes from Rust; a press sends the id
 * back and Rust decides whether anything is written. No preset number or name
 * is written here.
 */
export function DocumentPresets({ document: doc, onReturnFocus }: DocumentPresetsProps) {
  const base = useId();
  const { presets } = doc.view;
  const orientationCells: StripCell[] = [
    {
      id: "portrait",
      label: "Orientation Portrait",
      content: (
        <>
          <PageGlyph landscape={false} />
          Portrait
        </>
      ),
      tooltip: (
        <>
          <div>Taller than wide. Swaps width and height.</div>
          <div className="text-muted-foreground">
            Objects keep their place relative to the centre; nothing rotates.
          </div>
        </>
      ),
    },
    {
      id: "landscape",
      label: "Orientation Landscape",
      content: (
        <>
          <PageGlyph landscape />
          Landscape
        </>
      ),
      tooltip: (
        <>
          <div>Wider than tall. Swaps width and height.</div>
          <div className="text-muted-foreground">
            Objects keep their place relative to the centre; nothing rotates.
          </div>
        </>
      ),
    },
  ];
  return (
    <div className="flex flex-col gap-3">
      {presets.groupNames.map((name, group) => {
        const buttons = presets.presets.filter((preset) => preset.group === group);
        if (buttons.length === 0) {
          return null;
        }
        const headingId = `${base}-group-${group}`;
        const pressed = buttons.find((button) => button.pressed);
        return (
          <div key={headingId} className="flex flex-col gap-1">
            <h3 id={headingId} className="h-4 text-xs leading-4 font-semibold text-[var(--toolbar-icon)]">
              {name}
            </h3>
            <PresetStrip
              labelledBy={headingId}
              cells={buttons.map((button) => ({
                id: button.id,
                label: button.accessible,
                content: button.name,
                tooltip: tooltipOf(button.tooltip),
              }))}
              value={pressed?.id ?? null}
              onPress={(id) => doc.pickPreset(id)}
              onReturnFocus={onReturnFocus}
            />
          </div>
        );
      })}
      <div className="flex flex-col gap-1">
        <h3 id={`${base}-orientation`} className="h-4 text-xs leading-4 font-semibold text-[var(--toolbar-icon)]">
          Orientation
        </h3>
        <PresetStrip
          labelledBy={`${base}-orientation`}
          cells={orientationCells}
          value={presets.orientation === "none" ? null : presets.orientation}
          onPress={(id) => doc.setOrientation(id as "portrait" | "landscape")}
          onReturnFocus={onReturnFocus}
        />
      </div>
    </div>
  );
}
