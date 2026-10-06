import { useEffect, useLayoutEffect, useRef, useState } from "react";

import type { KeyHint } from "@/hooks/useEditorSession";
import { placeReadout } from "@/lib/readoutPlacement";

/** The chip's constant offset from the pointer, up and to the right: the
 * readout's own offset. */
const KEY_HINT_OFFSET_PX = 12;

interface KeyHintChipProps {
  hint: KeyHint | null;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * The one-line message of a key that could not act, for example R with
 * several objects selected (`specs/edit-interaction-polish/` criterion 59):
 * the hint chip's surface, text only, no pointer events, announced politely.
 * The hook clears the hint after 2 s.
 */
export function KeyHintChip({ hint, containerRef }: KeyHintChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  // The last pointer position over the canvas (canvas-relative CSS pixels),
  // tracked whether or not a hint is showing; `null` places the chip at the
  // canvas centre.
  const pointerRef = useRef<{ x: number; y: number } | null>(null);
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return undefined;
    }
    const track = (event: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      pointerRef.current = { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
    };
    container.addEventListener("pointermove", track);
    return () => container.removeEventListener("pointermove", track);
  }, [containerRef]);

  // Anchored once when a hint appears, at the pointer or the canvas centre.
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (hint && container) {
      setAnchor(
        pointerRef.current ?? { x: container.clientWidth / 2, y: container.clientHeight / 2 },
      );
    }
  }, [hint, containerRef]);

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
  }, [hint, anchor, containerRef]);

  if (!hint || !anchor) {
    return <div role="status" aria-live="polite" className="sr-only" />;
  }
  const placement = placeReadout(anchor, sizes.chip, sizes.canvas, KEY_HINT_OFFSET_PX);
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
      {hint.text}
    </div>
  );
}
