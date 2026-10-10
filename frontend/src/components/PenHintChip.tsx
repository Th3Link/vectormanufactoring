import { useEffect, useLayoutEffect, useRef, useState } from "react";

import {
  CHIP_REST_MS,
  chipIsImmediate,
  chipLines,
  hasChip,
  type PenCue,
} from "@/lib/penText";
import { placeAway, placeReadout } from "@/lib/readoutPlacement";

/** The chip's constant offset from the pointer at the moment it appears, up and to the right. */
const CHIP_OFFSET_PX = 12;

interface PenHintChipProps {
  cue: PenCue;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

interface Shown {
  /** The target the chip was placed for: a new target places it anew. */
  target: string;
  x: number;
  y: number;
}

/**
 * The two-line hint chip over a Pen target (`docs/design-system.md`, row "Pen hint chip"): it
 * says what the click will do and what Shift will do instead. Text only, takes no focus, is
 * `aria-hidden`. Continue and join targets show it after 600 ms of rest, or at once when Shift
 * changes over the target; a close target shows it at once. It is placed once, 12 px up and right
 * of the pointer (over a close or join target: on the side away from the path in progress, so it
 * covers neither the closing segment nor the handles), and stays there while the pointer stays on
 * the same target. It goes on leaving
 * the target, on a press and on any key but Shift.
 */
export function PenHintChip({ cue, containerRef }: PenHintChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const pointer = useRef({ x: 0, y: 0 });
  const [shown, setShown] = useState<Shown | null>(null);
  const [size, setSize] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });
  const previousKind = useRef("none");
  const previousTarget = useRef("");

  // The pointer, in the canvas's own coordinates.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return undefined;
    }
    const track = (event: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      pointer.current = { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
    };
    const hide = () => setShown(null);
    container.addEventListener("pointermove", track);
    container.addEventListener("pointerdown", hide);
    return () => {
      container.removeEventListener("pointermove", track);
      container.removeEventListener("pointerdown", hide);
    };
  }, [containerRef]);

  // Any key but Shift ends the chip.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Shift") {
        setShown(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const kindChanged = previousKind.current !== cue.kind;
    const targetChanged = previousTarget.current !== cue.target;
    previousKind.current = cue.kind;
    previousTarget.current = cue.target;
    if (!hasChip(cue)) {
      setShown(null);
      return undefined;
    }
    const target = cue.target === "" ? cue.kind : cue.target;
    const place = () => setShown({ target, x: pointer.current.x, y: pointer.current.y });
    if (chipIsImmediate(cue)) {
      // A close carries the choice: at once, placed once for the target.
      setShown((held) => (held?.target === target ? held : null));
      if (kindChanged || targetChanged) {
        place();
      }
      return undefined;
    }
    if (!targetChanged && kindChanged) {
      // Shift changed over the same target: the chip is asked for, at once.
      place();
      return undefined;
    }
    if (targetChanged) {
      setShown(null);
      const timer = window.setTimeout(place, CHIP_REST_MS);
      return () => window.clearTimeout(timer);
    }
    return undefined;
  }, [cue]);

  const lines = shown ? chipLines(cue) : null;
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
    setSize((previous) =>
      previous.chip.width === next.chip.width &&
      previous.chip.height === next.chip.height &&
      previous.canvas.width === next.canvas.width &&
      previous.canvas.height === next.canvas.height
        ? previous
        : next,
    );
  }, [shown, lines, containerRef]);

  if (!shown || !lines) {
    return null;
  }
  const away = { x: cue.awayX, y: cue.awayY };
  const placement =
    away.x === 0 && away.y === 0
      ? placeReadout(shown, size.chip, size.canvas, CHIP_OFFSET_PX)
      : placeAway(shown, size.chip, size.canvas, CHIP_OFFSET_PX, away);
  return (
    <div
      ref={chipRef}
      aria-hidden="true"
      className="pointer-events-none absolute z-10 rounded-md p-2 text-xs whitespace-nowrap"
      style={{
        left: placement.left,
        top: placement.top,
        background: "var(--toolbar-bg)",
        color: "var(--toolbar-icon)",
      }}
    >
      {lines.map((line, index) => (
        <div
          key={line}
          className={index === 0 ? "font-semibold" : undefined}
          style={index === 2 ? { color: "var(--panel-muted-fg)" } : undefined}
        >
          {line}
        </div>
      ))}
    </div>
  );
}
