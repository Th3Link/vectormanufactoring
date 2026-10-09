interface StatusBarProps {
  /** The cursor readout, e.g. "x: 12.3  y: 45.6 mm". */
  cursorText: string;
  /** The document size, e.g. "210.0 × 297.0 mm". */
  sizeText: string;
  /** The current zoom level's integer percentage (acceptance criterion
   * 9), range 2-8000. */
  zoomPercent: number;
}

/**
 * The permanent piece of chrome `project-file-foundation` added
 * (specs/0001-project-file-foundation: "Canvas and empty state") and
 * `canvas-navigation-and-selection` extends: left, the cursor position;
 * **center, the zoom percentage** (acceptance criterion 9); right, the
 * document size. Both texts come from the session
 * (`specs/0015-document-size-and-rulers/` criteria 12 and 21): fixed decimals
 * and the display unit written once, formatted in Rust.
 */
export function StatusBar({ cursorText, sizeText, zoomPercent }: StatusBarProps) {
  return (
    <div
      className="flex h-6 shrink-0 items-center justify-between px-2 text-xs"
      style={{ background: "var(--statusbar-bg)" }}
    >
      {/* `whitespace-pre` keeps the two spaces between x and y; `tabular-nums`
          keeps the unit at the end from moving with the digits. */}
      <span className="whitespace-pre tabular-nums">{cursorText}</span>
      <span>{zoomPercent}%</span>
      <span className="whitespace-pre tabular-nums">{sizeText}</span>
    </div>
  );
}
