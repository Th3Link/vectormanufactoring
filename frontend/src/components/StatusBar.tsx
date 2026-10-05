interface StatusBarProps {
  cursorMm: { x: number; y: number };
  sizeMm: { width: number; height: number };
  /** The current zoom level's integer percentage (acceptance criterion
   * 9), range 2-8000. */
  zoomPercent: number;
}

/**
 * The permanent piece of chrome `project-file-foundation` added
 * (specs/0001-project-file-foundation: "Canvas and empty state") and
 * `canvas-navigation-and-selection` extends: left, cursor position in
 * mm; **center, the zoom percentage** (acceptance criterion 9 — the
 * room `project-file-foundation` reserved for it); right, document size
 * in mm.
 */
export function StatusBar({ cursorMm, sizeMm, zoomPercent }: StatusBarProps) {
  return (
    <div
      className="flex h-6 shrink-0 items-center justify-between px-2 text-xs"
      style={{ background: "var(--statusbar-bg)" }}
    >
      <span>
        x: {cursorMm.x.toFixed(1)}  y: {cursorMm.y.toFixed(1)}
      </span>
      <span>{zoomPercent}%</span>
      <span>
        {sizeMm.width.toFixed(1)} × {sizeMm.height.toFixed(1)} mm
      </span>
    </div>
  );
}
