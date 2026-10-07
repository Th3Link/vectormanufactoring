/**
 * Where to put the live numeric readout chip
 * (`specs/0005-object-transform/specification.md`, UX notes; the
 * design-system "Live transform readout" row): normally `offset` px up and
 * to the right of the pointer, but never off the canvas — flipped to the
 * left of the pointer when it would cross the right edge, flipped below
 * it when it would cross the top edge, then clamped inside the canvas as
 * a last resort (a canvas smaller than the chip).
 */
export interface Size {
  width: number;
  height: number;
}

export interface Placement {
  left: number;
  top: number;
}

export function placeReadout(
  anchor: { x: number; y: number },
  chip: Size,
  canvas: Size,
  offset: number,
): Placement {
  let left = anchor.x + offset;
  let top = anchor.y - offset - chip.height;
  if (left + chip.width > canvas.width) {
    left = anchor.x - offset - chip.width;
  }
  if (top < 0) {
    top = anchor.y + offset;
  }
  return {
    left: Math.max(0, Math.min(left, canvas.width - chip.width)),
    top: Math.max(0, Math.min(top, canvas.height - chip.height)),
  };
}

/** Distance from a handle's glyph edge to the entry chip, in CSS pixels
 * (`docs/design-system.md`, "Transform entry chip": 10px clear space). */
const ENTRY_CHIP_GAP_PX = 10;

/** Half the largest handle glyph (the 12px rotate arrow), in CSS pixels. */
const HANDLE_GLYPH_RADIUS_PX = 6;

function clampInside(left: number, top: number, chip: Size, canvas: Size): Placement {
  return {
    left: Math.max(0, Math.min(left, canvas.width - chip.width)),
    top: Math.max(0, Math.min(top, canvas.height - chip.height)),
  };
}

function coversPoint(placement: Placement, chip: Size, point: { x: number; y: number }): boolean {
  const margin = HANDLE_GLYPH_RADIUS_PX;
  return (
    point.x >= placement.left - margin &&
    point.x <= placement.left + chip.width + margin &&
    point.y >= placement.top - margin &&
    point.y <= placement.top + chip.height + margin
  );
}

/**
 * Where to put the typed-entry chip (`object-transform-refinements` UX
 * notes, "Numeric entry control"): upright, centred outward from the handle
 * along the line from the box centre through it, with 10px clear of the
 * handle's glyph edge (`glyphReach`: further out where a skew arrow sits
 * on the same side), then clamped inside the canvas. If the clamp would
 * cover the handle it flips to the inner side. While the handle is off
 * screen the chip stays clamped at the canvas edge.
 */
export function placeEntryChip(
  handle: { x: number; y: number },
  center: { x: number; y: number },
  chip: Size,
  canvas: Size,
  glyphReach = HANDLE_GLYPH_RADIUS_PX,
): Placement {
  let dx = handle.x - center.x;
  let dy = handle.y - center.y;
  const length = Math.hypot(dx, dy);
  if (length < 1e-6) {
    dx = 0;
    dy = -1;
  } else {
    dx /= length;
    dy /= length;
  }
  const at = (sign: number): Placement => {
    // The chip's half-extent along the direction, so the nearest point of
    // the chip (not its centre) is the gap away from the glyph.
    const reach =
      glyphReach +
      ENTRY_CHIP_GAP_PX +
      (Math.abs(dx) * chip.width) / 2 +
      (Math.abs(dy) * chip.height) / 2;
    const cx = handle.x + sign * dx * reach;
    const cy = handle.y + sign * dy * reach;
    return clampInside(cx - chip.width / 2, cy - chip.height / 2, chip, canvas);
  };
  const outward = at(1);
  return coversPoint(outward, chip, handle) ? at(-1) : outward;
}

/** Distance of the typed-move chip's near corner from the centre glyph, in
 * CSS pixels, on both axes (`docs/design-system.md`, "Move entry chip"). */
const MOVE_CHIP_OFFSET_PX = 16;

/**
 * Where to put the typed-move chip (`edit-interaction-polish` criterion 18):
 * its top left 16px right of and 16px below the centre glyph, so it never
 * covers the glyph, flipped to the left of the glyph when it would cross the
 * right edge and above it when it would cross the bottom edge (the readout's
 * rule, mirrored), then clamped inside the canvas.
 */
export function placeMoveChip(
  center: { x: number; y: number },
  chip: Size,
  canvas: Size,
): Placement {
  let left = center.x + MOVE_CHIP_OFFSET_PX;
  let top = center.y + MOVE_CHIP_OFFSET_PX;
  if (left + chip.width > canvas.width) {
    left = center.x - MOVE_CHIP_OFFSET_PX - chip.width;
  }
  if (top + chip.height > canvas.height) {
    top = center.y - MOVE_CHIP_OFFSET_PX - chip.height;
  }
  return clampInside(left, top, chip, canvas);
}
