import { StyleRow } from "@/components/panel/StyleRow";
import { ValueField } from "@/components/panel/ValueField";
import { PLACE_OPTIONS, markerOptions } from "@/components/panel/styleOptions";
import { ToggleGroup } from "@/components/ui/toggle-group";
import type { MarkerSlotName, StylePanelApi } from "@/hooks/useStylePanel";

const COUNT_MESSAGES = { count: "Enter a whole number from 1 to 500" } as const;

const SLOTS: readonly { slot: MarkerSlotName; label: string; group: string }[] = [
  { slot: "start", label: "Start", group: "Start marker" },
  { slot: "mid", label: "Middle", group: "Middle marker" },
  { slot: "end", label: "End", group: "End marker" },
];

interface MarkerRowsProps {
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/**
 * The Markers group under Cap (`specs/0018-stroke-markers` criteria 1, 2, 24,
 * 30 to 32, UX notes): a title, Start, Middle and End each None / Arrow / Dot,
 * Place while Middle is set, Count while Place is Spaced, and a muted line when
 * every selected path is closed. Every choice is one commit on the click; the
 * rows are not in the tree while the stroke is off or no path is selected.
 */
export function MarkerRows({ panel, onReturnFocus }: MarkerRowsProps) {
  const { view } = panel;
  const values = { start: view.markerStart, mid: view.markerMid, end: view.markerEnd };
  return (
    <div className="contents">
      <h4 className="mt-2 h-4 text-xs leading-4 font-semibold text-[var(--toolbar-icon)]">
        Markers
      </h4>
      {SLOTS.map(({ slot, label, group }) => (
        <StyleRow key={slot} label={label}>
          <ToggleGroup
            label={group}
            options={markerOptions(slot)}
            value={values[slot] === "mixed" ? null : values[slot]}
            onChange={(shape) => panel.setMarkerShape(slot, shape)}
            itemWidth={40}
            onReturnFocus={onReturnFocus}
          />
        </StyleRow>
      ))}
      {view.markerClosedNote && (
        <p className="text-xs text-[var(--panel-muted-fg)]">Closed paths have no start or end.</p>
      )}
      {view.markerPlaceShown && (
        <StyleRow label="Place">
          <ToggleGroup
            label="Marker placement"
            options={PLACE_OPTIONS}
            value={view.markerPlace === "mixed" ? null : view.markerPlace}
            onChange={panel.setMarkerPlace}
            itemWidth={88}
            onReturnFocus={onReturnFocus}
          />
        </StyleRow>
      )}
      {view.markerCountShown && (
        <ValueField
          label="Count"
          name="Marker count"
          field="marker-count"
          text={view.markerCountText}
          bar={view.markerCountBar}
          mixed={view.markerCountMixed}
          resettable={view.markerCountResettable}
          unit=""
          valueNow={view.markerCount}
          unitWords="markers"
          valueMin={1}
          typedMax={view.countTypedMax}
          defaultText={view.countDefaultText}
          messages={COUNT_MESSAGES}
          panel={panel}
          onReturnFocus={onReturnFocus}
        />
      )}
    </div>
  );
}
