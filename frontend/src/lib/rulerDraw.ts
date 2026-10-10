// Draws one ruler strip from the tick layout `curvyo-ui-core` computed
// (`specs/0015-document-size-and-rulers/`, `docs/design-system.md` rows
// "Ruler", "Ruler ticks", "Ruler labels", "Ruler pointer marker"). Every
// number and label comes from Rust; this file only turns positions into
// device-pixel rectangles and text.

/** The few Canvas2D members the drawing uses, so a test can record them. */
export interface RulerContext {
  fillStyle: CanvasRenderingContext2D["fillStyle"];
  font: string;
  textBaseline: CanvasTextBaseline;
  textAlign: CanvasTextAlign;
  fillRect(x: number, y: number, width: number, height: number): void;
  fillText(text: string, x: number, y: number): void;
  save(): void;
  restore(): void;
  translate(x: number, y: number): void;
  rotate(angle: number): void;
}

/** What `WasmSession.ruler_view` returns, as plain data. */
export interface RulerData {
  majors: ArrayLike<number>;
  minors: ArrayLike<number>;
  origin: number | undefined;
  labelTicks: ArrayLike<number>;
  labelTexts: ArrayLike<string>;
}

/** The colours, already resolved from the design-system tokens. */
export interface RulerColours {
  background: string;
  tick: string;
  tickMinor: string;
  label: string;
  edge: string;
  pointer: string;
}

export interface RulerStrip {
  /** `true` for the ruler along the top edge. */
  horizontal: boolean;
  /** The strip's CSS size: length along the axis, thickness across it. */
  length: number;
  thickness: number;
  devicePixelRatio: number;
  /** The pointer's position along the strip, CSS px, or `null` when it is
   * outside the canvas. */
  pointer: number | null;
}

/** The rulers' thickness and the corner's size, px (`docs/design-system.md`,
 * "Viewport and rulers"). Also the grid track size in `App.tsx`. */
export const RULER_THICKNESS_PX = 24;

/** The depth of a minor tick, CSS px (design system: 8 px). */
export const MINOR_TICK_DEPTH = 8;
/** Where a label starts relative to its tick, CSS px. */
export const LABEL_OFFSET = 4;
/** A label's distance from the strip's outer edge, CSS px. */
export const LABEL_INSET = 3;
/** The label font size, CSS px (`text-xs`). */
export const LABEL_SIZE = 12;

/** Real minus sign in labels is already U+2212 (Rust builds the text). */
export function labelFont(devicePixelRatio: number): string {
  return `${LABEL_SIZE * devicePixelRatio}px system-ui, sans-serif`;
}

/** Whole device pixels for a CSS length: at least one. */
function devicePixels(css: number, dpr: number): number {
  return Math.max(1, Math.round(css * dpr));
}

/**
 * Paints the strip. Coordinates are device pixels: a tick's position is
 * rounded to a whole device pixel so it is never two half-tone columns.
 * The canvas-facing edge is the bottom of the horizontal ruler and the right
 * edge of the vertical one; ticks grow from it.
 */
export function drawRuler(
  ctx: RulerContext,
  data: RulerData,
  strip: RulerStrip,
  colours: RulerColours,
): void {
  const { horizontal, devicePixelRatio: dpr } = strip;
  const length = Math.round(strip.length * dpr);
  const thickness = Math.round(strip.thickness * dpr);
  const hair = devicePixels(1, dpr);

  // A rectangle along the axis: `at` and `span` along it, `depth` across it,
  // growing from the canvas-facing edge.
  const bar = (at: number, span: number, depth: number, colour: string) => {
    ctx.fillStyle = colour;
    if (horizontal) {
      ctx.fillRect(at, thickness - depth, span, depth);
    } else {
      ctx.fillRect(thickness - depth, at, depth, span);
    }
  };
  // The first device pixel of a mark `span` device pixels wide whose centre
  // is on `px`: the rounded left edge, not the rounded centre, so the mark is
  // never more than half a device pixel from the position it stands for.
  const tickAt = (px: number, span: number) => Math.round(px * dpr - span / 2);

  bar(0, length, thickness, colours.background);

  for (let i = 0; i < data.minors.length; i += 1) {
    bar(
      tickAt(data.minors[i], hair),
      hair,
      devicePixels(MINOR_TICK_DEPTH, dpr),
      colours.tickMinor,
    );
  }
  for (let i = 0; i < data.majors.length; i += 1) {
    const isOrigin =
      data.origin !== undefined && Math.abs(data.majors[i] - data.origin) < 0.5;
    const span = isOrigin ? hair * 2 : hair;
    bar(tickAt(data.majors[i], span), span, thickness, colours.tick);
  }

  if (strip.pointer !== null) {
    bar(tickAt(strip.pointer, hair), hair, thickness, colours.pointer);
  }

  // The edge line on the canvas-facing side, over the ticks.
  bar(0, length, hair, colours.edge);

  ctx.fillStyle = colours.label;
  ctx.font = labelFont(dpr);
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  for (let i = 0; i < data.labelTicks.length; i += 1) {
    const text = data.labelTexts[i];
    const start = (data.labelTicks[i] + LABEL_OFFSET) * dpr;
    if (horizontal) {
      ctx.fillText(text, Math.round(start), LABEL_INSET * dpr);
    } else {
      // Rotated 90 degrees counter-clockwise: reads bottom to top. Right
      // aligned at the point 4 px after the tick, so the end of the text is
      // next to the tick and the text begins its own width further down.
      ctx.save();
      ctx.translate(LABEL_INSET * dpr, Math.round(start));
      ctx.rotate(-Math.PI / 2);
      ctx.textAlign = "right";
      ctx.fillText(text, 0, 0);
      ctx.restore();
    }
  }
}
