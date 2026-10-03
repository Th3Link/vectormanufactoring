interface StatusBarProps {
  cursorMm: { x: number; y: number };
  sizeMm: { width: number; height: number };
}

/**
 * The one permanent piece of chrome this slice adds
 * (specs/project-file-foundation: "Canvas and empty state"). Left: cursor
 * position in mm. Right: document size in mm. Room is left for a zoom
 * control later, but nothing non-functional is added now.
 */
export function StatusBar({ cursorMm, sizeMm }: StatusBarProps) {
  return (
    <div
      className="flex h-6 shrink-0 items-center justify-between px-2 text-xs"
      style={{ background: "var(--statusbar-bg)" }}
    >
      <span>
        x: {cursorMm.x.toFixed(1)}  y: {cursorMm.y.toFixed(1)}
      </span>
      <span>
        {sizeMm.width.toFixed(1)} × {sizeMm.height.toFixed(1)} mm
      </span>
    </div>
  );
}
