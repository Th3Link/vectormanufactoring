import type { PathCommand } from "@/lib/pathText";

/**
 * The glyphs of Combine and Break apart (`docs/design-system.md`, row "Command glyphs"): 16 x 16
 * viewBox drawn at 20px, 1.5px stroke kept absolute, solid shapes, `currentColor`.
 *
 * Combine is one solid rounded square (11 x 11) with a concentric square hole (5 x 5, evenodd):
 * one object with a hole. Break apart is a smaller solid square with a hole (8 x 8, hole 3 x 3)
 * at the upper left and, 1.6 away from it, a separate solid 4 x 4 square at the lower right: the
 * plate keeps its hole, only the separate piece leaves.
 */

const STROKE = {
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinejoin: "round" as const,
  vectorEffect: "non-scaling-stroke" as const,
};

/** The 11 x 11 square at (2.5, 2.5), corners rounded by 1, and its 5 x 5 hole at (5.5, 5.5). */
const COMBINED =
  "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z M5.5 5.5V10.5H10.5V5.5Z";

/** The 8 x 8 plate at (1.5, 1.5), corners rounded by 1, and its 3 x 3 hole at (4, 4). */
const PLATE =
  "M2.5 1.5H8.5A1 1 0 0 1 9.5 2.5V8.5A1 1 0 0 1 8.5 9.5H2.5A1 1 0 0 1 1.5 8.5V2.5A1 1 0 0 1 2.5 1.5Z M4 4V7H7V4Z";

/** The separate 4 x 4 piece, 1.6 from the plate's corner: at (11.1, 11.1). */
const PIECE = "M11.1 11.1H15.1V15.1H11.1Z";

function shapes(command: PathCommand) {
  if (command === "combine") {
    return <path d={COMBINED} fill="currentColor" fillRule="evenodd" {...STROKE} />;
  }
  return (
    <>
      <path d={PLATE} fill="currentColor" fillRule="evenodd" {...STROKE} />
      <path d={PIECE} fill="currentColor" {...STROKE} />
    </>
  );
}

/** The glyph of `command`, 20px, in the colour of the text around it. */
export function PathGlyph({ command }: { command: PathCommand }) {
  return (
    <svg width={20} height={20} viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      {shapes(command)}
    </svg>
  );
}
