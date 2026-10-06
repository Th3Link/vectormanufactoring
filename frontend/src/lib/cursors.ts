/**
 * Canvas cursors for the Select tool's transform handles
 * (`specs/0005-object-transform/specification.md`, UX notes, "Cursor
 * feedback"). `vecmanf-editor-wasm`'s `cursor_hint()` returns a plain
 * string; this turns it into a CSS `cursor` value — all the geometry
 * (which handle, which angle) is decided in Rust, nothing here.
 *
 * - `"resize:<deg>"`: one custom double-headed-arrow image, rotated live
 *   to `<deg>` (clockwise from horizontal) so it points the way the
 *   handle will actually move the shape, with the nearest of the four
 *   built-in resize cursors as the fallback for platforms that ignore
 *   custom cursor images.
 * - `"rotate"`: a circular-arrow image that never rotates.
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
