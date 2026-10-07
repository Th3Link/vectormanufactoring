/**
 * Canvas cursors for the Select tool's transform handles
 * (`specs/0005-object-transform/specification.md`, UX notes, "Cursor
 * feedback"). `curvyo-editor-wasm`'s `cursor_hint()` returns a plain
 * string; this turns it into a CSS `cursor` value — all the geometry
 * (which handle, which angle) is decided in Rust, nothing here.
 *
 * - `"resize:<deg>"`: one custom double-headed-arrow image, rotated live
 *   to `<deg>` (clockwise from horizontal) so it points the way the
 *   handle will actually move the shape, with the nearest of the four
 *   built-in resize cursors as the fallback for platforms that ignore
 *   custom cursor images.
 * - `"rotate"`: a circular-arrow image that never rotates (all eight
 *   rotate handles, `object-transform-refinements`).
 * - `"skew:<deg>"`: two opposed parallel arrows, rotated live like the
 *   resize cursor (the box rotation for the top and bottom handles, plus 90°
 *   for left and right), with `ew-resize`/`ns-resize` as the fallback.
 * - `"move"`: the built-in `move` cursor (the centre handle).
 * - `"pointer"`: the built-in `pointer` cursor, hovering and dragging a
 *   parameter handle (`specs/unified-object-editing/` criterion 5): not a
 *   resize or move cursor, so it does not promise one.
 * - anything else: no override (the tool's normal cursor).
 */

const SIZE = 24;
const HOTSPOT = SIZE / 2;

/** An SVG drawn white-under-black so it reads on any background. */
function svgCursor(inner: string, hotspotFallback: string): string {
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${SIZE}" height="${SIZE}" viewBox="0 0 ${SIZE} ${SIZE}">` +
    inner +
    `</svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") ${HOTSPOT} ${HOTSPOT}, ${hotspotFallback}`;
}

/** Draws `d` twice: a thick white halo, then a thin black stroke. */
function outlined(d: string): string {
  const common = `d="${d}" fill="none" stroke-linecap="round" stroke-linejoin="round"`;
  return (
    `<path ${common} stroke="white" stroke-width="4"/>` +
    `<path ${common} stroke="black" stroke-width="1.5"/>`
  );
}

const DOUBLE_ARROW = "M4 12 H20 M4 12 L8 8.5 M4 12 L8 15.5 M20 12 L16 8.5 M20 12 L16 15.5";
/** Two opposed parallel arrows: the upper one points right, the lower left. */
const SHEAR_ARROWS =
  "M4 9 H20 M20 9 L16.5 6 M20 9 L16.5 12 M20 15 H4 M4 15 L7.5 12 M4 15 L7.5 18";
const CIRCULAR_ARROW = "M17.5 8 A7 7 0 1 0 19 12.5 M17.5 8 L17.5 3.5 M17.5 8 L13 8";

/** The built-in resize cursor nearest to a double arrow at `degrees`. */
export function nearestBuiltInResizeCursor(degrees: number): string {
  const normalized = ((degrees % 180) + 180) % 180;
  if (normalized < 22.5 || normalized >= 157.5) {
    return "ew-resize";
  }
  if (normalized < 67.5) {
    return "nwse-resize";
  }
  if (normalized < 112.5) {
    return "ns-resize";
  }
  return "nesw-resize";
}

/** Parses a `cursor_hint()` string into a CSS cursor, or `undefined` for
 * "leave the tool's normal cursor alone". */
export function cursorForHint(hint: string): string | undefined {
  if (hint === "rotate") {
    return svgCursor(outlined(CIRCULAR_ARROW), "grab");
  }
  if (hint === "move") {
    return "move";
  }
  if (hint === "pointer") {
    return "pointer";
  }
  if (hint.startsWith("skew:")) {
    const degrees = Number.parseFloat(hint.slice("skew:".length));
    if (!Number.isFinite(degrees)) {
      return undefined;
    }
    const rotated = `<g transform="rotate(${degrees} ${HOTSPOT} ${HOTSPOT})">${outlined(SHEAR_ARROWS)}</g>`;
    // A built-in fallback by the nearest axis: along x, else along y.
    const normalized = ((degrees % 180) + 180) % 180;
    const fallback = normalized < 45 || normalized >= 135 ? "ew-resize" : "ns-resize";
    return svgCursor(rotated, fallback);
  }
  if (hint.startsWith("resize:")) {
    const degrees = Number.parseFloat(hint.slice("resize:".length));
    if (!Number.isFinite(degrees)) {
      return undefined;
    }
    const rotated = `<g transform="rotate(${degrees} ${HOTSPOT} ${HOTSPOT})">${outlined(DOUBLE_ARROW)}</g>`;
    return svgCursor(rotated, nearestBuiltInResizeCursor(degrees));
  }
  return undefined;
}
