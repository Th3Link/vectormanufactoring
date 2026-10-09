import type { BooleanOp } from "@/lib/booleanText";

/**
 * The glyphs of the five boolean operations (`docs/design-system.md`, row
 * "Boolean glyphs"): two 9 x 9 squares on a 16 x 16 viewBox, the first-drawn
 * (the lower object) at (1.75, 1.75), the second at (5.25, 5.25), so the
 * overlap is 5.5 x 5.5. Solid is the result, outline only is what
 * disappears. Drawn at 20px; the stroke is 1.5px on screen at any scale.
 *
 * Every shape is a path of its own (no clip, no mask), so no element needs
 * an id and the strokes sit exactly on the edges of the result.
 */

const LOWER = { x: 1.75, y: 1.75, width: 9, height: 9, rx: 1 };
const UPPER = { x: 5.25, y: 5.25, width: 9, height: 9, rx: 1 };

/** The lower square minus the overlap (an L), corners rounded by 1. */
const LOWER_MINUS_OVERLAP =
  "M2.75 1.75H9.75A1 1 0 0 1 10.75 2.75V5.25H5.25V10.75H2.75A1 1 0 0 1 1.75 9.75V2.75A1 1 0 0 1 2.75 1.75Z";

/** The upper square minus the overlap: the lower one turned by 180 degrees
 * about the centre of the viewBox. */
const UPPER_MINUS_OVERLAP =
  "M13.25 14.25H6.25A1 1 0 0 1 5.25 13.25V10.75H10.75V5.25H13.25A1 1 0 0 1 14.25 6.25V13.25A1 1 0 0 1 13.25 14.25Z";

/** The overlap, with the corners of the squares it takes them from. */
const OVERLAP =
  "M6.25 5.25H10.75V9.75A1 1 0 0 1 9.75 10.75H5.25V6.25A1 1 0 0 1 6.25 5.25Z";

const STROKE = {
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinejoin: "round" as const,
  vectorEffect: "non-scaling-stroke" as const,
};

function Solid({ d }: { d: string }) {
  return <path d={d} fill="currentColor" {...STROKE} />;
}

function Outline({ square }: { square: typeof LOWER }) {
  return <rect {...square} fill="none" {...STROKE} />;
}

function glyphShapes(op: BooleanOp) {
  switch (op) {
    case "union":
      return (
        <>
          <rect {...LOWER} fill="currentColor" {...STROKE} />
          <rect {...UPPER} fill="currentColor" {...STROKE} />
        </>
      );
    case "difference":
      return (
        <>
          <Solid d={LOWER_MINUS_OVERLAP} />
          <Outline square={UPPER} />
        </>
      );
    case "intersection":
      return (
        <>
          <Outline square={LOWER} />
          <Outline square={UPPER} />
          <Solid d={OVERLAP} />
        </>
      );
    case "exclusion":
      return (
        <>
          <Solid d={LOWER_MINUS_OVERLAP} />
          <Solid d={UPPER_MINUS_OVERLAP} />
        </>
      );
    case "reverse_difference":
      return (
        <>
          <Outline square={LOWER} />
          <Solid d={UPPER_MINUS_OVERLAP} />
        </>
      );
  }
}

/** The glyph of `op`, 20px, in the colour of the text around it. */
export function BooleanGlyph({ op }: { op: BooleanOp }) {
  return (
    <svg width={20} height={20} viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      {glyphShapes(op)}
    </svg>
  );
}
