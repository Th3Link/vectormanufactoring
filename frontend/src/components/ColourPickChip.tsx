import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { Swatch } from "@/components/panel/Swatch";
import type { WasmSession } from "@/lib/editorSession";
import { placeReadout } from "@/lib/readoutPlacement";

/** The chip's offset from the pointer, up and to the right. */
const OFFSET_PX = 12;

interface Hover {
  x: number;
  y: number;
  /** `#RRGGBBAA`, or empty where nothing is painted. */
  hex: string;
  paint: string;
}

interface ColourPickChipProps {
  /** Picking is on (the cursor is the eyedropper). */
  active: boolean;
  getSession: () => WasmSession | null;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/** `#RRGGBBAA` as the packed colour and alpha percent the swatch takes. */
function parseHex(hex: string): { rgb: number; opacity: number } {
  const rgb = Number.parseInt(hex.slice(1, 7), 16);
  const alpha = Number.parseInt(hex.slice(7, 9), 16);
  return { rgb, opacity: (alpha / 255) * 100 };
}

/**
 * The eyedropper's readout (`specs/0017-style-panel-rework` criterion 23, UX
 * notes section 5): while picking, a small chip beside the pointer shows the
 * colour a click would take as `#RRGGBBAA` with the paint it comes from, or
 * "No paint here". It is the existing readout surface: text and a swatch, no
 * control, no focus, no pointer events. The colour is read from the session at
 * most once per animation frame, after the canvas has processed the move.
 */
export function ColourPickChip({ active, getSession, containerRef }: ColourPickChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<Hover | null>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  useEffect(() => {
    const container = containerRef.current;
    if (!active || !container) {
      setHover(null);
      return undefined;
    }
    let frame = 0;
    let last: { x: number; y: number } | null = null;
    const read = () => {
      frame = 0;
      const session = getSession();
      if (!last || !session) {
        return;
      }
      const [hex = "", paint = ""] = session.colour_pick_hover();
      setHover({ ...last, hex, paint });
    };
    const track = (event: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      last = { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
      if (frame === 0) {
        frame = window.requestAnimationFrame(read);
      }
    };
    const leave = () => {
      last = null;
      setHover(null);
    };
    container.addEventListener("pointermove", track);
    container.addEventListener("pointerleave", leave);
    return () => {
      window.cancelAnimationFrame(frame);
      container.removeEventListener("pointermove", track);
      container.removeEventListener("pointerleave", leave);
      setHover(null);
    };
  }, [active, getSession, containerRef]);

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
  }, [hover, containerRef]);

  if (!active || !hover) {
    return null;
  }
  const placement = placeReadout(hover, sizes.chip, sizes.canvas, OFFSET_PX);
  const colour = hover.hex ? parseHex(hover.hex) : null;
  return (
    <div
      ref={chipRef}
      aria-hidden
      className="pointer-events-none absolute z-40 flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-xs whitespace-nowrap tabular-nums"
      style={{
        left: placement.left,
        top: placement.top,
        background: "var(--toolbar-bg)",
        color: "var(--toolbar-icon)",
        border: "1px solid color-mix(in srgb, var(--toolbar-icon) 25%, transparent)",
      }}
    >
      {colour ? (
        <>
          <Swatch rgb={colour.rgb} opacity={colour.opacity} mixed={false} size={16} />
          <span>{hover.hex}</span>
          <span className="text-[var(--panel-muted-fg)]">{hover.paint}</span>
        </>
      ) : (
        <span>No paint here</span>
      )}
    </div>
  );
}
