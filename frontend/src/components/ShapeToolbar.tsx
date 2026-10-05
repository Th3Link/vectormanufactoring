import type { PolyStarMode, Tool } from "@/hooks/useEditorSession";

export interface ShapeToolbarProps {
  tool: Tool;
  polyStarMode: PolyStarMode;
  polyStarPointCount: number;
  polyStarRatio: number;
  onSetPolyStarMode: (mode: PolyStarMode) => void;
  onSetPolyStarPointCount: (count: number) => void;
  /** The ratio slider's live, uncommitted preview — call on every
   * tick while dragging (architect review: never commit per tick). */
  onPreviewPolyStarRatio: (ratio: number) => void;
  /** Commits whatever `onPreviewPolyStarRatio` has accumulated — call
   * once, when the drag on the slider ends. */
  onCommitPolyStarRatio: () => void;
  onRemoveCornerRounding: () => void;
  onConvertSelectedToPaths: () => void;
}

const MIN_POINT_COUNT = 3;
const MAX_POINT_COUNT = 1024;

/**
 * The contextual tool-options bar for the three shape tools
 * (`specs/0003-primitive-shapes/specification.md`'s UX notes, "Tool rail
 * additions and shortcuts" / "Polygon/star point-count and ratio"):
 * shown directly under the main menu, full width, the same row and
 * mechanism as `NodeToolbar`'s own bar — only the controls relevant to
 * the active shape tool are shown.
 *
 * "Object to path" (acceptance criteria 17, 21, 22) lives here for all
 * three shape tools, since it is the one action every primitive shares
 * regardless of kind; the per-shape controls (mode/point-count/ratio,
 * remove-rounding) are each scoped to their own tool.
 */
export function ShapeToolbar({
  tool,
  polyStarMode,
  polyStarPointCount,
  polyStarRatio,
  onSetPolyStarMode,
  onSetPolyStarPointCount,
  onPreviewPolyStarRatio,
  onCommitPolyStarRatio,
  onRemoveCornerRounding,
  onConvertSelectedToPaths,
}: ShapeToolbarProps) {
  return (
    <div
      className="flex h-9 shrink-0 items-center gap-3 border-b border-border px-2 text-sm"
      style={{ background: "var(--toolbar-bg)" }}
    >
      {tool === "rectangle" && (
        <button
          type="button"
          onClick={onRemoveCornerRounding}
          className="rounded-md px-2 py-1 text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-ring"
        >
          Remove rounding
        </button>
      )}

      {tool === "polygon-star" && (
        <>
          <div
            role="group"
            aria-label="Polygon or star mode"
            className="flex overflow-hidden rounded-md ring-1 ring-border"
          >
            <button
              type="button"
              aria-pressed={polyStarMode === "polygon"}
              onClick={() => onSetPolyStarMode("polygon")}
              className="px-2 py-1"
              style={{
                background:
                  polyStarMode === "polygon" ? "var(--toolbar-icon-active-bg)" : "transparent",
                color:
                  polyStarMode === "polygon" ? "var(--toolbar-icon-active-fg)" : "var(--toolbar-icon)",
              }}
            >
              Polygon
            </button>
            <button
              type="button"
              aria-pressed={polyStarMode === "star"}
              onClick={() => onSetPolyStarMode("star")}
              className="px-2 py-1"
              style={{
                background: polyStarMode === "star" ? "var(--toolbar-icon-active-bg)" : "transparent",
                color: polyStarMode === "star" ? "var(--toolbar-icon-active-fg)" : "var(--toolbar-icon)",
              }}
            >
              Star
            </button>
          </div>

          <label className="flex items-center gap-1 text-[var(--toolbar-icon)]">
            Points
            <input
              type="number"
              min={MIN_POINT_COUNT}
              max={MAX_POINT_COUNT}
              step={1}
              value={polyStarPointCount}
              onChange={(event) => {
                const next = Math.round(Number(event.target.value));
                if (Number.isFinite(next) && next >= MIN_POINT_COUNT && next <= MAX_POINT_COUNT) {
                  onSetPolyStarPointCount(next);
                }
              }}
              className="w-16 rounded-md border border-border bg-transparent px-1 py-0.5"
            />
          </label>

          {polyStarMode === "star" && (
            <label className="flex items-center gap-1 text-[var(--toolbar-icon)]">
              Ratio
              <input
                type="range"
                min={0.01}
                max={0.99}
                step={0.01}
                value={polyStarRatio}
                // Every tick only previews live (architect review: a
                // slider must not commit once per tick, ADR 0002 §9) —
                // the real commit happens once, on release/blur/key-up,
                // below.
                onChange={(event) => onPreviewPolyStarRatio(Number(event.target.value))}
                onPointerUp={onCommitPolyStarRatio}
                onKeyUp={onCommitPolyStarRatio}
                onBlur={onCommitPolyStarRatio}
              />
              <span className="w-10 text-right">{polyStarRatio.toFixed(2)}</span>
            </label>
          )}
        </>
      )}

      <button
        type="button"
        onClick={onConvertSelectedToPaths}
        className="ml-auto rounded-md px-2 py-1 text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-ring"
      >
        Object to path
      </button>
    </div>
  );
}
