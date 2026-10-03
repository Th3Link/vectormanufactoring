import { useCallback } from "react";

interface CanvasProps {
  /** Called with the pointer position relative to the canvas's top-left. */
  onPointerPositionChange: (position: { x: number; y: number }) => void;
}

/**
 * The empty canvas (specs/project-file-foundation: "Canvas and empty
 * state"). Fills the window body edge-to-edge, no margin/border/frame —
 * later panels dock beside or over it, they never shrink it.
 *
 * No real view transform exists yet (no pan/zoom, no document render —
 * that is `vecmanf-render-core`/`vecmanf-editor-wasm`, explicitly out of
 * scope for this slice per `specs/project-file-foundation/adrs.md`), so
 * the position reported here is a nominal 1 CSS pixel = 1 mm placeholder
 * just to drive the status bar's "x / y" readout. The first tool story
 * replaces this with the real document-space transform (ADR 0002 §4,
 * Y-down mm).
 */
export function Canvas({ onPointerPositionChange }: CanvasProps) {
  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const bounds = event.currentTarget.getBoundingClientRect();
      onPointerPositionChange({
        x: event.clientX - bounds.left,
        y: event.clientY - bounds.top,
      });
    },
    [onPointerPositionChange],
  );

  return (
    <div
      className="flex-1"
      style={{ background: "var(--canvas-bg)" }}
      onPointerMove={handlePointerMove}
    />
  );
}
