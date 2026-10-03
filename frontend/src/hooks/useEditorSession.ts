import { useCallback, useEffect, useRef, useState } from "react";

import { createSession } from "@/lib/editorSession";
import type { WasmSession } from "@/lib/editorSession";

/** Which tool is active (`specification.md`'s tool rail: Pen or Node). */
export type Tool = "pen" | "node";

/** A plain-JS copy of the Rust `NodeToolbarState` — read out of the
 * wasm-bindgen struct instance once, immediately, so the instance itself
 * can be `free()`d rather than held onto (see `readToolbarState` below). */
export interface NodeToolbarState {
  canInsert: boolean;
  canDelete: boolean;
  canConvertToCorner: boolean;
  canConvertToSmooth: boolean;
  canMakeLine: boolean;
  canMakeCurve: boolean;
}

const EMPTY_TOOLBAR_STATE: NodeToolbarState = {
  canInsert: false,
  canDelete: false,
  canConvertToCorner: false,
  canConvertToSmooth: false,
  canMakeLine: false,
  canMakeCurve: false,
};

/** A hand-rolled double-click detector (specs/path-node-editing/
 * adrs.md's `PenTool` doc comment: "double-click detection itself is the
 * frontend's job"). A real DOM `dblclick` event fires only after BOTH
 * clicks' own `pointerdown`/`pointerup` pairs have already run — relaying
 * both to the session would place an unwanted extra node (acceptance
 * criterion 3: a finishing double-click is "instead of placing another
 * node", not in addition to it). This tracks press timing/position
 * itself instead, so the second press of a double-click can be withheld
 * from the session entirely and read as "finish"/"insert" once its
 * matching release arrives. The first press still committed normally —
 * exactly the node the maker expects to end up with (matching Inkscape).
 */
const DOUBLE_CLICK_MS = 400;
const DOUBLE_CLICK_PX = 5;

function readToolbarState(raw: {
  can_insert: boolean;
  can_delete: boolean;
  can_convert_to_corner: boolean;
  can_convert_to_smooth: boolean;
  can_make_line: boolean;
  can_make_curve: boolean;
  free(): void;
}): NodeToolbarState {
  const state: NodeToolbarState = {
    canInsert: raw.can_insert,
    canDelete: raw.can_delete,
    canConvertToCorner: raw.can_convert_to_corner,
    canConvertToSmooth: raw.can_convert_to_smooth,
    canMakeLine: raw.can_make_line,
    canMakeCurve: raw.can_make_curve,
  };
  raw.free();
  return state;
}

export interface EditorSession {
  /** Attach to the `<canvas>` element the host renders. */
  canvasRef: React.RefObject<HTMLCanvasElement | null>;
  /** Attach to the canvas's focusable container (canvas-focus shortcut
   * scope, docs/design-system.md's keyboard shortcut table). */
  containerRef: React.RefObject<HTMLDivElement | null>;
  tool: Tool;
  nodeToolbarState: NodeToolbarState;
  setTool: (tool: Tool) => void;
  escape: () => void;
  deleteSelected: () => void;
  convertSelected: (kind: "corner" | "smooth") => void;
  makeLine: () => void;
  makeCurve: () => void;
  insertSelected: () => void;
  finishPen: () => void;
  onPointerDown: (event: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerMove: (event: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerUp: (event: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerLeave: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
}

/** Owns one `WasmSession` for the app's lifetime: creates it, attaches
 * it to the host's `<canvas>`, keeps its `wgpu` surface sized to the
 * canvas (specs/path-node-editing/adrs.md's PASS note, requirement 2),
 * runs its per-frame render loop, and forwards pointer/keyboard input —
 * "the frontend renders state and forwards input events into it; it
 * holds no editing logic of its own" (same file, ADR 0001 §1/§2). */
export function useEditorSession(
  onCursorMove: (point: { x: number; y: number }) => void,
): EditorSession {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<WasmSession | null>(null);
  const lastPressRef = useRef<{ time: number; x: number; y: number } | null>(
    null,
  );
  const suppressedPressRef = useRef(false);

  const [tool, setToolState] = useState<Tool>("pen");
  const [nodeToolbarState, setNodeToolbarState] = useState<NodeToolbarState>(
    EMPTY_TOOLBAR_STATE,
  );

  /** Re-reads `tool`/`node_toolbar_state` from the session after any
   * call that might have changed them — cheap, and simpler than having
   * every call site know which ones to refresh. */
  const syncFromSession = useCallback(() => {
    const session = sessionRef.current;
    if (!session) {
      return;
    }
    setToolState(session.tool() === "node" ? "node" : "pen");
    setNodeToolbarState(readToolbarState(session.node_toolbar_state()));
  }, []);

  useEffect(() => {
    let cancelled = false;
    let frame = 0;
    let resizeObserver: ResizeObserver | null = null;

    createSession().then((session) => {
      if (cancelled) {
        session.free();
        return;
      }
      sessionRef.current = session;

      const canvas = canvasRef.current;
      if (!canvas) {
        return;
      }

      const attach = async () => {
        const width = Math.max(1, Math.round(canvas.clientWidth));
        const height = Math.max(1, Math.round(canvas.clientHeight));
        canvas.width = width;
        canvas.height = height;
        await session.attach_canvas(canvas, width, height);
        if (cancelled) {
          return;
        }
        // scale=1 (screen px per document mm), origin (0,0): this
        // slice has no pan/zoom UI yet (specs/path-node-editing/
        // adrs.md's ViewTransform decision), so document space and
        // the canvas's own CSS pixels coincide 1:1 — the same
        // placeholder convention project-file-foundation's Canvas
        // used, now backed by the real transform type.
        session.set_view(1.0, 0.0, 0.0);
        syncFromSession();

        resizeObserver = new ResizeObserver((entries) => {
          const entry = entries[0];
          if (!entry) {
            return;
          }
          const nextWidth = Math.max(1, Math.round(entry.contentRect.width));
          const nextHeight = Math.max(
            1,
            Math.round(entry.contentRect.height),
          );
          canvas.width = nextWidth;
          canvas.height = nextHeight;
          session.resize(nextWidth, nextHeight);
        });
        resizeObserver.observe(canvas);

        const renderLoop = () => {
          session.render();
          frame = requestAnimationFrame(renderLoop);
        };
        frame = requestAnimationFrame(renderLoop);
      };
      void attach();
    });

    return () => {
      cancelled = true;
      if (frame) {
        cancelAnimationFrame(frame);
      }
      resizeObserver?.disconnect();
      sessionRef.current?.free();
      sessionRef.current = null;
    };
  }, [syncFromSession]);

  const setTool = useCallback(
    (next: Tool) => {
      sessionRef.current?.set_tool(next);
      syncFromSession();
    },
    [syncFromSession],
  );

  const escape = useCallback(() => {
    sessionRef.current?.escape();
    syncFromSession();
  }, [syncFromSession]);

  const deleteSelected = useCallback(() => {
    sessionRef.current?.delete_selected();
    syncFromSession();
  }, [syncFromSession]);

  const convertSelected = useCallback(
    (kind: "corner" | "smooth") => {
      sessionRef.current?.convert_selected(kind);
      syncFromSession();
    },
    [syncFromSession],
  );

  const makeLine = useCallback(() => {
    sessionRef.current?.make_line();
    syncFromSession();
  }, [syncFromSession]);

  const makeCurve = useCallback(() => {
    sessionRef.current?.make_curve();
    syncFromSession();
  }, [syncFromSession]);

  const insertSelected = useCallback(() => {
    sessionRef.current?.insert_selected();
    syncFromSession();
  }, [syncFromSession]);

  const finishPen = useCallback(() => {
    sessionRef.current?.finish_pen();
    syncFromSession();
  }, [syncFromSession]);

  /** The pointer's document-space position, given `scale=1`/`origin=
   * (0,0)` (see the attach effect above): identical to its position
   * relative to the canvas's own top-left corner. */
  const documentPoint = useCallback((event: { clientX: number; clientY: number }) => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return { x: 0, y: 0 };
    }
    const bounds = canvas.getBoundingClientRect();
    return { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
  }, []);

  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      const session = sessionRef.current;
      if (!session) {
        return;
      }
      const { x, y } = documentPoint(event);
      const now = performance.now();
      const last = lastPressRef.current;
      const isDoubleClick =
        last !== null &&
        now - last.time < DOUBLE_CLICK_MS &&
        Math.hypot(x - last.x, y - last.y) < DOUBLE_CLICK_PX;
      lastPressRef.current = { time: now, x, y };
      suppressedPressRef.current = isDoubleClick;
      if (isDoubleClick) {
        return;
      }
      session.pointer_down(x, y, event.shiftKey);
      syncFromSession();
    },
    [documentPoint, syncFromSession],
  );

  const onPointerMove = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      const session = sessionRef.current;
      const { x, y } = documentPoint(event);
      onCursorMove({ x, y });
      session?.pointer_hover(x, y);
    },
    [documentPoint, onCursorMove],
  );

  const onPointerUp = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      const session = sessionRef.current;
      if (!session) {
        return;
      }
      const { x, y } = documentPoint(event);
      if (suppressedPressRef.current) {
        suppressedPressRef.current = false;
        if (tool === "pen") {
          session.finish_pen();
        } else {
          session.insert_at(x, y);
        }
      } else {
        session.pointer_up(x, y);
      }
      syncFromSession();
    },
    [documentPoint, syncFromSession, tool],
  );

  const onPointerLeave = useCallback(() => {
    sessionRef.current?.pointer_leave();
  }, []);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      switch (event.key) {
        case "b":
        case "B":
          setTool("pen");
          break;
        case "n":
        case "N":
          setTool("node");
          break;
        case "Enter":
          if (tool === "pen") {
            finishPen();
          }
          break;
        case "Escape":
          escape();
          break;
        case "Delete":
        case "Backspace":
          event.preventDefault();
          deleteSelected();
          break;
        default:
          return;
      }
    },
    [deleteSelected, escape, finishPen, setTool, tool],
  );

  return {
    canvasRef,
    containerRef,
    tool,
    nodeToolbarState,
    setTool,
    escape,
    deleteSelected,
    convertSelected,
    makeLine,
    makeCurve,
    insertSelected,
    finishPen,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerLeave,
    onKeyDown,
  };
}
