import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { placeReadout } from "@/lib/readoutPlacement";

/** How long the hint stays, unless a press, a key or the pointer leaving
 * dismisses it first (`specs/unified-object-editing/` criterion 32). */
const EDIT_HINT_MS = 3000;

/** The chip's constant offset from the pointer, up and to the right: the
 * readout's own offset. */
const EDIT_HINT_OFFSET_PX = 12;

const LINES = [
  "Drag a handle to edit this shape",
  "Double-click a handle to type a value",
  "Nodes: Object to path, then double-click",
];

export interface EditHint {
  /** Canvas-relative CSS pixels of the double-click. */
  x: number;
  y: number;
  /** Changes on every double-click, so the 3 s life restarts. */
  id: number;
}

interface EditHintChipProps {
  hint: EditHint | null;
  containerRef: React.RefObject<HTMLDivElement | null>;
  onDismiss: () => void;
}

/**
 * The hint after a double-click on a primitive's outline, body or centre
 * handle, where nothing else happens (`docs/design-system.md`, "Edit hint
 * chip"): the hint chip's surface with three lines, shown for 3 seconds or
 * until a press, a key or the pointer leaving. Text only, ignores pointer
 * events, writes nothing, announced politely.
 */
export function EditHintChip({ hint, containerRef, onDismiss }: EditHintChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  useEffect(() => {
    if (!hint) {
      return undefined;
    }
    const timer = window.setTimeout(onDismiss, EDIT_HINT_MS);
    const container = containerRef.current;
    container?.addEventListener("pointerdown", onDismiss);
    container?.addEventListener("pointerleave", onDismiss);
    window.addEventListener("keydown", onDismiss);
    return () => {
      window.clearTimeout(timer);
      container?.removeEventListener("pointerdown", onDismiss);
      container?.removeEventListener("pointerleave", onDismiss);
      window.removeEventListener("keydown", onDismiss);
    };
  }, [hint, containerRef, onDismiss]);

  useLayoutEffect(() => {
    const chip = chipRef.current;
    const container = containerRef.current;
    if (!chip || !container) {
      return;
    }
    const next = {
      chip: { width: chip.offsetWidth, height: chip.offsetHeight },
      canvas: { width: container.clientWidth, height: container.clientHeight },
    };
    setSizes((previous) =>
      previous.chip.width === next.chip.width &&
      previous.chip.height === next.chip.height &&
      previous.canvas.width === next.canvas.width &&
      previous.canvas.height === next.canvas.height
        ? previous
        : next,
    );
  }, [hint, containerRef]);

  if (!hint) {
    return <div role="status" aria-live="polite" className="sr-only" />;
  }
  const placement = placeReadout(hint, sizes.chip, sizes.canvas, EDIT_HINT_OFFSET_PX);
  return (
    <div
      ref={chipRef}
      role="status"
      aria-live="polite"
      className="pointer-events-none absolute z-10 rounded-md p-2 text-xs whitespace-nowrap"
      style={{
        left: placement.left,
        top: placement.top,
        background: "var(--toolbar-bg)",
        color: "var(--toolbar-icon)",
      }}
    >
      {LINES.map((line, index) => (
        <div key={line} className={index === 0 ? "font-medium" : undefined}>
          {line}
        </div>
      ))}
    </div>
  );
}
