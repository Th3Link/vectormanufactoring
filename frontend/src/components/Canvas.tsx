import { NodeContextMenu } from "@/components/NodeToolbar";
import type { EditorSession } from "@/hooks/useEditorSession";
import { cursorForHint } from "@/lib/cursors";

/** The readout's constant screen-space offset from its anchor, up and to
 * the right (`specs/0005-object-transform/specification.md`'s UX notes:
 * 12px, anchored to the pointer so it stays readable while a rotate drag
 * swings the handle through an arc). */
const READOUT_OFFSET_PX = 12;

interface CanvasProps {
  editor: EditorSession;
}

/**
 * Hosts the `vecmanf-editor-wasm` module's `<canvas>` (ADR 0001 §3):
 * forwards pointer/keyboard input into the attached `WasmSession` and
 * lets it render every frame — no editing logic lives here, matching
 * `vecmanf-ui-core`'s own "the frontend... holds no editing logic of its
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
          style={{ background: "var(--canvas-bg)" }}
        />
      </NodeContextMenu>
      {editor.liveReadout && (
        // On-canvas, not status-bar (`specs/0003-primitive-shapes/
        // specification.md`'s "Live creation feedback": "direct
        // manipulation keeps the number where the maker's eyes already
        // are"), positioned near point B — the live drag endpoint.
        // `editor.liveReadout.x`/`y` already arrive as canvas-relative
        // CSS pixels (`specs/0004-canvas-navigation-and-selection/
        // adrs.md`: "Anything the DOM positions... is returned already
        // converted"), so this positions the overlay directly, with no
        // conversion math here.
        <div
          className="pointer-events-none absolute z-10 -translate-y-full rounded-md px-1.5 py-0.5 text-xs"
          style={{
            left: editor.liveReadout.x + READOUT_OFFSET_PX,
            top: editor.liveReadout.y - READOUT_OFFSET_PX,
            background: "var(--toolbar-bg)",
            color: "var(--toolbar-icon)",
          }}
        >
          {editor.liveReadout.text}
        </div>
      )}
    </div>
  );
}
