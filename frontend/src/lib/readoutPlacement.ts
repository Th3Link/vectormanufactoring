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
