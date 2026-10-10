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
  /** Changes with the zoom, so a zoom by key re-reads the colour too. */
  viewKey: number;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/** How long a pick's sentence stays in the status region, ms: long enough to be
 * spoken, short enough that the same sentence after the next pick is a change. */
const ANNOUNCEMENT_MS = 4000;

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
 *
 * Where the pick would be the document background (`specs/0040-document-
 * background` criterion 35) the chip reads swatch, the muted word "Background",
 * then the stored colour; for an object's paint it is the colour, then the muted
 * "stroke" or "fill". A pick that ended picking is said once in the polite status
 * region (criterion 39).
 */
export function ColourPickChip({
  active,
  getSession,
  viewKey,
  containerRef,
}: ColourPickChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<Hover | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const announceTimer = useRef(0);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  // The pointer is tracked whether or not picking is on: the press on the
  // eyedropper button happens elsewhere, and the first move over the canvas
  // arrives before `active` does.
  const pointer = useRef<{ x: number; y: number } | null>(null);
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return undefined;
    }
    const track = (event: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      pointer.current = { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
    };
    const leave = () => {
      pointer.current = null;
    };
    // A press that picked a colour has a sentence for the status region; one that did not
    // (a cancel, a press with picking off) has none. Read after the canvas handled the press.
    let frame = 0;
    const announce = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const sentence = getSession()?.take_colour_pick_announcement() ?? "";
        if (sentence !== "") {
          setAnnouncement(sentence);
          window.clearTimeout(announceTimer.current);
          announceTimer.current = window.setTimeout(() => setAnnouncement(""), ANNOUNCEMENT_MS);
        }
      });
    };
    container.addEventListener("pointermove", track, true);
    container.addEventListener("pointerleave", leave);
    container.addEventListener("pointerup", announce);
    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(announceTimer.current);
      container.removeEventListener("pointermove", track, true);
      container.removeEventListener("pointerleave", leave);
      container.removeEventListener("pointerup", announce);
    };
  }, [containerRef, getSession]);

  useEffect(() => {
    const container = containerRef.current;
    if (!active || !container) {
      setHover(null);
      return undefined;
    }
    let frame = 0;
    const read = () => {
      frame = 0;
      const session = getSession();
      const last = pointer.current;
      if (!last || !session) {
        setHover(null);
        return;
      }
      const [hex = "", paint = ""] = session.colour_pick_hover();
      setHover({ ...last, hex, paint });
    };
    // Once per animation frame, after the canvas has processed the event.
    const schedule = () => {
      if (frame === 0) {
        frame = window.requestAnimationFrame(read);
      }
    };
    // A pan or zoom moves the document under a still pointer.
    container.addEventListener("pointermove", schedule);
    container.addEventListener("wheel", schedule);
    container.addEventListener("pointerleave", schedule);
    schedule();
    return () => {
      window.cancelAnimationFrame(frame);
      container.removeEventListener("pointermove", schedule);
      container.removeEventListener("wheel", schedule);
      container.removeEventListener("pointerleave", schedule);
      setHover(null);
    };
  }, [active, getSession, viewKey, containerRef]);

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

  const status = (
    <div role="status" aria-live="polite" className="sr-only">
      {announcement}
    </div>
  );
  if (!active || !hover) {
    return status;
  }
  const placement = placeReadout(hover, sizes.chip, sizes.canvas, OFFSET_PX);
  const colour = hover.hex ? parseHex(hover.hex) : null;
  const background = hover.paint === "background";
  return (
    <>
      {status}
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
          <span className="flex gap-1">
            {background && <span className="text-[var(--panel-muted-fg)]">Background</span>}
            <span>{hover.hex}</span>
            {!background && <span className="text-[var(--panel-muted-fg)]">{hover.paint}</span>}
          </span>
        </>
      ) : (
        <span>No paint here</span>
      )}
    </div>
    </>
  );
}
