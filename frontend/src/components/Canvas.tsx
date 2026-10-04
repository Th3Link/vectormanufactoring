import { NodeContextMenu } from "@/components/NodeToolbar";
import type { EditorSession } from "@/hooks/useEditorSession";

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
 * (`specs/project-file-foundation`'s "chrome never frames the canvas"
 * precedent, reaffirmed by `docs/design-system.md`).
 */
export function Canvas({ editor }: CanvasProps) {
  return (
    <div
      ref={editor.containerRef}
      tabIndex={0}
      onKeyDown={editor.onKeyDown}
      className={`flex-1 outline-none ${
        editor.tool === "pen"
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
    </div>
  );
}
