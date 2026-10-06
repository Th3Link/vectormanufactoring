import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { placeReadout } from "@/lib/readoutPlacement";

/** How long the pointer must rest on a handle before the hint shows
 * (`object-transform-refinements` criterion 54). */
const HINT_DELAY_MS = 600;

/** The hint's constant offset from the pointer, up and to the right — the
 * readout chip's own offset. */
const HINT_OFFSET_PX = 12;

/** One short line each: the handle's name, then its modifiers. */
const HINT_LINES: Record<string, string[]> = {
  "resize-edge": ["Resize", "Shift: from center", "Double-click: type a size"],
  "resize-corner": [
    "Resize",
    "Shift: from center",
    "Ctrl: keep proportions",
    "Double-click: type a size",
  ],
  "resize-corner-uniform": ["Resize", "Shift: from center", "Double-click: type a size"],
  "rotate-corner": [
    "Rotate",
    "Shift: pivot at opposite corner",
    "Ctrl: snap",
    "Double-click: type an angle",
  ],
  "rotate-side": [
    "Rotate",
    "Pivot: opposite side",
    "Ctrl: snap",
    "Double-click: type an angle",
  ],
  skew: ["Skew", "Shift: from the center line", "Ctrl: snap"],
  move: ["Move"],
  "param-radius": ["Corner radius", "Double-click: type a value"],
  "param-inner": ["Inner radius", "Double-click: type a ratio"],
};

interface HandleHintChipProps {
  /** `vecmanf-editor-wasm`'s `handle_hint()`: which handle the pointer is on. */
  hint: string;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * The hover hint for a transform handle (`specs/object-transform-
 * refinements/specification.md` criterion 54; `docs/design-system.md`,
 * "Transform handle hint chip"): text only, shown after the pointer has
 * rested on a handle for 600 ms, anchored once at the pointer (12 px up and
 * right, the readout's placement and flipping), never following it, taking
 * no pointer events, gone on a press, on leaving the handle or on any key.
 */
export function HandleHintChip({ hint, containerRef }: HandleHintChipProps) {
  const lines = HINT_LINES[hint];
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null);
  const pointerRef = useRef<{ x: number; y: number } | null>(null);
  const chipRef = useRef<HTMLDivElement>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  // Always track the pointer, hint or not: the move that makes a hint
  // appear happens before this component's hint effect is registered, so
  // recording only while a hint is active left the anchor stale or empty.
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

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !lines) {
      return undefined;
    }
    let timer: number | undefined;
    const arm = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        // No known pointer position: no hint rather than one at a stale spot.
        if (pointerRef.current) {
          setAnchor(pointerRef.current);
        }
      }, HINT_DELAY_MS);
    };
    const hide = () => {
      window.clearTimeout(timer);
      setAnchor(null);
    };
    const onMove = () => {
      // Any movement is not resting: hide and wait for the pointer to stop.
      hide();
      arm();
    };
    arm();
    container.addEventListener("pointermove", onMove);
    container.addEventListener("pointerdown", hide);
    window.addEventListener("keydown", hide);
    return () => {
      window.clearTimeout(timer);
      container.removeEventListener("pointermove", onMove);
      container.removeEventListener("pointerdown", hide);
      window.removeEventListener("keydown", hide);
      setAnchor(null);
    };
  }, [hint, lines, containerRef]);

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
  }, [anchor, hint, containerRef]);

  if (!lines || !anchor) {
    return null;
  }
  const placement = placeReadout(anchor, sizes.chip, sizes.canvas, HINT_OFFSET_PX);
  return (
    <div
      ref={chipRef}
      className="pointer-events-none absolute z-10 rounded-md p-2 text-xs whitespace-nowrap"
      style={{
        left: placement.left,
        top: placement.top,
        background: "var(--toolbar-bg)",
        color: "var(--toolbar-icon)",
      }}
    >
      {lines.map((line, index) => (
        <div key={line} className={index === 0 ? "font-medium" : undefined}>
          {line}
        </div>
      ))}
    </div>
  );
}
