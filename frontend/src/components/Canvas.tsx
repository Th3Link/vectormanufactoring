import { useLayoutEffect, useRef, useState } from "react";

import { EditHintChip } from "@/components/EditHintChip";
import { HandleHintChip } from "@/components/HandleHintChip";
import { KeyHintChip } from "@/components/KeyHintChip";
import { MoveBadges } from "@/components/MoveBadges";
import { NodeContextMenu } from "@/components/NodeToolbar";
import { MoveEntryChip } from "@/components/MoveEntryChip";
import { SelectionAnnouncer } from "@/components/SelectionAnnouncer";
import { TransformEntryChip } from "@/components/TransformEntryChip";
import type { EditorSession } from "@/hooks/useEditorSession";
import { cursorForHint } from "@/lib/cursors";
import { type Rect, placeReadout } from "@/lib/readoutPlacement";

/** The readout's constant screen-space offset from its anchor, up and to
 * the right (`specs/0005-object-transform/specification.md`'s UX notes:
 * 12px, anchored to the pointer so it stays readable while a rotate drag
 * swings the handle through an arc). */
const READOUT_OFFSET_PX = 12;

/** The tool rail's clearance from the canvas's left edge, px: the readout is
 * never drawn under it. */
const TOOL_RAIL_CLEAR_PX = 64;
/** The properties panel's collapse tab sits on the canvas's right edge: a chip
 * never reaches into the last 14 px, so it cannot cover the tab when the
 * pointer leaves the canvas over the panel. */
const PANEL_TAB_CLEAR_PX = 14;

interface CanvasProps {
  editor: EditorSession;
}

/**
 * Hosts the `curvyo-editor-wasm` module's `<canvas>` (ADR 0001 §3):
 * forwards pointer/keyboard input into the attached `WasmSession` and
 * lets it render every frame — no editing logic lives here, matching
 * `curvyo-ui-core`'s own "the frontend... holds no editing logic of its
 * own" (ADR 0001 §1/§2).
 *
 * Fills the window body edge-to-edge, no margin/border/frame — the tool
 * rail and contextual toolbar dock beside/above it, they never shrink it
 * (`specs/0001-project-file-foundation`'s "chrome never frames the canvas"
 * precedent, reaffirmed by `docs/design-system.md`).
 */
export function Canvas({ editor }: CanvasProps) {
  return (
    <div
      ref={editor.containerRef}
      tabIndex={0}
      onKeyDown={editor.onKeyDown}
      onKeyUp={editor.onKeyUp}
      onBlur={editor.onContainerBlur}
      // Transform-handle cursors (`object-transform`): only the Select
      // tool ever reports a non-default hint, and a pan gesture (grab
      // cursors above) always wins.
      style={
        editor.isPanning || editor.isSpaceHeld
          ? undefined
          : { cursor: cursorForHint(editor.cursorHint) }
      }
      className={`relative flex-1 outline-none ${
        // Pan cursor convention (`docs/design-system.md`): grabbing for
        // the duration of a drag-pan, open-hand from the moment Space is
        // held (even before any drag motion) — both override whatever
        // cursor the active tool would otherwise show, for exactly the
        // pan's duration (acceptance criteria 3, 4).
        editor.isPanning
          ? "cursor-grabbing"
          : editor.isSpaceHeld
            ? "cursor-grab"
            : editor.tool === "pen"
              ? editor.isHoveringPenCloseTarget
                ? "canvas-cursor-pen-close"
                : "canvas-cursor-pen"
              : editor.tool === "rectangle" ||
                  editor.tool === "ellipse" ||
                  editor.tool === "polygon-star"
                ? // The creation tools show their crosshair everywhere, over
                  // existing objects too (`unified-object-editing` criterion 25).
                  "cursor-crosshair"
                : "cursor-default"
      }`}
    >
      {/* The context-menu wrapper stays mounted across both tools —
          see NodeContextMenu's own doc comment: swapping it in/out by
          tool would move <canvas> to a different tree position and
          React would remount it, dropping the WasmSession's attached
          wgpu surface. */}
      <NodeContextMenu
        disabled={editor.tool !== "node"}
        state={editor.nodeToolbarState}
        actions={{
          insertSelected: editor.insertSelected,
          deleteSelected: editor.deleteSelected,
          convertSelected: editor.convertSelected,
          makeLine: editor.makeLine,
          makeCurve: editor.makeCurve,
          joinSelected: editor.joinSelected,
          splitSelected: editor.splitSelected,
        }}
      >
        <canvas
          ref={editor.canvasRef}
          className="block size-full"
          onPointerDown={editor.onPointerDown}
          onPointerMove={editor.onPointerMove}
          onPointerUp={editor.onPointerUp}
          onPointerLeave={editor.onPointerLeave}
          onPointerCancel={editor.onPointerCancel}
          onContextMenu={(event) => {
            // The pen and shape tools have no context menu of their own
            // (UX notes: the node tool's six actions are the only
            // ones); suppress the browser's native menu either way so a
            // right-click never interrupts drawing. `NodeContextMenu`
            // itself prevents default for its own (node-tool-only) menu.
            if (editor.tool !== "node") {
              event.preventDefault();
            }
          }}
          style={{ background: "var(--pasteboard-bg)" }}
        />
      </NodeContextMenu>
      {editor.transformEntry && (
        // Keyed by the handle so a different entry starts with fresh text,
        // while a zoom or pan (same handle, new position) keeps what was typed.
        <TransformEntryChip
          key={`${editor.transformEntry.kind}:${editor.transformEntry.fields
            .map((f) => f.name + f.prefill)
            .join("|")}`}
          entry={editor.transformEntry}
          containerRef={editor.containerRef}
          onCommit={editor.commitTransformEntry}
          onCancel={editor.cancelTransformEntry}
          onLinked={editor.transformEntryLinked}
          onNote={editor.entryConversionNote}
        />
      )}
      {editor.moveEntry && (
        <MoveEntryChip
          entry={editor.moveEntry}
          containerRef={editor.containerRef}
          onCommit={editor.commitMoveEntry}
          onCancel={editor.cancelTransformEntry}
        />
      )}
      <HandleHintChip
        hint={editor.handleHint}
        cornerLines={editor.cornerHintLines}
        conversionCounts={editor.conversionHoverCounts}
        containerRef={editor.containerRef}
      />
      <KeyHintChip hint={editor.keyHint} containerRef={editor.containerRef} />
      <SelectionAnnouncer text={editor.selectionAnnouncement} />
      <EditHintChip
        hint={editor.editHint}
        polygon={editor.selectBar.pointsShown && !editor.selectBar.ratioShown}
        containerRef={editor.containerRef}
        onDismiss={editor.dismissEditHint}
      />
      <MoveBadges badges={editor.moveBadges} containerRef={editor.containerRef} />
      {editor.liveReadout && (
        <ReadoutChip
          text={editor.liveReadout.text}
          note={editor.liveReadout.note}
          x={editor.liveReadout.x}
          y={editor.liveReadout.y}
          containerRef={editor.containerRef}
        />
      )}
    </div>
  );
}

/** The floating Select bar's rectangle in the canvas's own coordinates, or
 * `null` when no bar is shown. */
function measureBar(container: HTMLElement): Rect | null {
  const bar = container.parentElement?.querySelector("[data-context-bar]");
  if (!bar) {
    return null;
  }
  const outer = container.getBoundingClientRect();
  const box = bar.getBoundingClientRect();
  return {
    left: box.left - outer.left,
    top: box.top - outer.top,
    right: box.right - outer.left,
    bottom: box.bottom - outer.top,
  };
}

function sameRect(a: Rect | null, b: Rect | null): boolean {
  if (a === null || b === null) {
    return a === b;
  }
  return a.left === b.left && a.top === b.top && a.right === b.right && a.bottom === b.bottom;
}

interface ReadoutChipProps {
  text: string;
  /** Canvas-relative CSS pixels, already converted by Rust
   * (`specs/0004-canvas-navigation-and-selection/adrs.md`: "Anything the
   * DOM positions... is returned already converted"). */
  x: number;
  y: number;
  /** A second line under the numbers ("2 shapes become paths"). */
  note?: string;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * The on-canvas numeric readout (`specs/0003-primitive-shapes/
 * specification.md`'s "Live creation feedback": "direct manipulation keeps
 * the number where the maker's eyes already are"; the transform readouts
 * of `specs/0005-object-transform` use the same chip). Measured after each
 * render so it can be kept fully inside the canvas — flipped left or below
 * the pointer near the right and top edges — instead of vanishing there.
 */
function ReadoutChip({ text, x, y, note, containerRef }: ReadoutChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: {
      width: Number.POSITIVE_INFINITY,
      height: Number.POSITIVE_INFINITY,
    },
    bar: null as Rect | null,
  });

  // Both sizes are measured in a layout effect (never read from a ref
  // during render), before the browser paints, so the chip is placed
  // correctly on the frame it first shows.
  useLayoutEffect(() => {
    const chip = chipRef.current;
    const container = containerRef.current;
    if (!chip || !container) {
      return;
    }
    const next = {
      chip: { width: chip.offsetWidth, height: chip.offsetHeight },
      canvas: { width: container.clientWidth, height: container.clientHeight },
      bar: measureBar(container),
    };
    setSizes((previous) =>
      previous.chip.width === next.chip.width &&
      previous.chip.height === next.chip.height &&
      previous.canvas.width === next.canvas.width &&
      previous.canvas.height === next.canvas.height &&
      sameRect(previous.bar, next.bar)
        ? previous
        : next,
    );
  }, [text, x, y, note, containerRef]);

  const placement = placeReadout(
    { x, y },
    sizes.chip,
    { width: sizes.canvas.width - PANEL_TAB_CLEAR_PX, height: sizes.canvas.height },
    READOUT_OFFSET_PX,
    sizes.bar ?? undefined,
  );

  return (
    <div
      ref={chipRef}
      className="pointer-events-none absolute z-40 whitespace-nowrap rounded-md px-1.5 py-0.5 text-xs"
      style={{
        left: Math.max(placement.left, TOOL_RAIL_CLEAR_PX),
        top: placement.top,
        background: "var(--toolbar-bg)",
        color: "var(--toolbar-icon)",
        // A hairline so the chip reads as its own surface wherever it lands,
        // also over the Select bar, which shares its ground.
        border: "1px solid color-mix(in srgb, var(--toolbar-icon) 25%, transparent)",
      }}
    >
      {text}
      {note !== undefined && <div className="opacity-80">{note}</div>}
    </div>
  );
}
