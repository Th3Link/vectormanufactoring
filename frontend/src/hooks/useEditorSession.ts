import { useCallback, useEffect, useRef, useState } from "react";

import { createSession, openSession } from "@/lib/editorSession";
import type { WasmSession } from "@/lib/editorSession";

/** Screen pixels per document millimetre this slice's placeholder view
 * uses (`specs/path-node-editing/adrs.md`'s `ViewTransform` decision: no
 * pan/zoom UI yet, so this is the whole view transform). `96 / 25.4` is
 * CSS's own "1in == 96px" convention expressed per millimetre — close
 * enough to a real screen's pixel density to make this slice's one fixed
 * stroke width (0.25mm, acceptance criterion 6) and node/handle glyphs
 * actually visible, unlike a literal 1:1 mm:px scale (0.25px is
 * sub-pixel on every real display). */
const CSS_PX_PER_MM = 96 / 25.4;

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
  /** Acceptance criterion 5's cursor cue: whether the live cursor is
   * over the in-progress pen path's own close target — `Canvas` swaps
   * to the "pen-with-small-circle" cursor variant while this is `true`. */
  isHoveringPenCloseTarget: boolean;
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
  /** File → New: swaps in a brand-new, empty session. */
  newProject: () => void;
  /** File → Open / the OS file association: parses `bytes` as a `.vmf`
   * and swaps it in as the live session.
   * @throws the exact user-facing sentence naming why `bytes` could not
   * be opened (see `@/lib/editorSession`'s `openSession`) — the caller
   * shows it, this hook does not. */
  openProject: (bytes: Uint8Array) => Promise<void>;
  /** File → Save / Save As: packs the live session's document into
   * `.vmf` container bytes for the host to write.
   * @throws a plain `Error` if there is no live session yet, or the
   * `JsValue` `vecmanf-editor-wasm`'s `pack` throws if the document could
   * not be serialized — the caller shows it, this hook does not. */
  packProject: (appVersion: string) => Uint8Array;
}

/** Owns one `WasmSession` for the app's lifetime: creates it, attaches
 * it to the host's `<canvas>`, keeps its `wgpu` surface sized to the
 * canvas (specs/path-node-editing/adrs.md's PASS note, requirement 2),
 * runs its per-frame render loop, and forwards pointer/keyboard input —
 * "the frontend renders state and forwards input events into it; it
 * holds no editing logic of its own" (same file, ADR 0001 §1/§2).
 *
 * The session itself is swappable: `newProject`/`openProject` free
 * whatever is currently attached and attach a different one in its
 * place (`specs/path-node-editing/adrs.md`'s PR review: "the host does
 * byte I/O only" — Open/New used to recreate the native `Document`;
 * now they recreate this hook's `WasmSession`), without tearing down
 * the mount effect's resize observer or render loop, which both always
 * read `sessionRef.current` fresh rather than closing over one instance. */
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
  const [isHoveringPenCloseTarget, setIsHoveringPenCloseTarget] =
    useState(false);

  /** Re-reads `tool`/`node_toolbar_state`/the pen close-target hover cue
   * from the session after any call that might have changed them —
   * cheap, and simpler than having every call site know which ones to
   * refresh. */
  const syncFromSession = useCallback(() => {
    const session = sessionRef.current;
    if (!session) {
      return;
    }
    setToolState(session.tool() === "node" ? "node" : "pen");
    setNodeToolbarState(readToolbarState(session.node_toolbar_state()));
    setIsHoveringPenCloseTarget(session.is_hovering_pen_close_target());
  }, []);

  /** Frees whatever session is currently attached (if any), makes
   * `session` the live one, and attaches it to the host's `<canvas>` —
   * the mount effect's own first attach and every later New/Open swap
   * all go through this one path.
   *
   * Known limitation: if a second call starts before the first's
   * `attach_canvas` (async: it negotiates a `wgpu` adapter/device) has
   * resolved, the first's session is freed out from under that in-
   * flight call rather than the call being awaited or cancelled first —
   * a pre-existing hazard (the original mount-only version of this
   * effect had the same race against unmount), not something this
   * swap support introduces. Rapid repeated New/Open clicks are the
   * only way to hit it; no acceptance criterion exercises that. */
  const attachSession = useCallback(
    async (session: WasmSession) => {
      sessionRef.current?.free();
      sessionRef.current = session;

      const canvas = canvasRef.current;
      if (!canvas) {
        return;
      }
      const width = Math.max(1, Math.round(canvas.clientWidth));
      const height = Math.max(1, Math.round(canvas.clientHeight));
      canvas.width = width;
      canvas.height = height;
      await session.attach_canvas(canvas, width, height);
      if (sessionRef.current !== session) {
        // Superseded by another New/Open (or the hook unmounted) while
        // `attach_canvas` was in flight; whichever call superseded this
        // one already owns `sessionRef.current`, and this `session` has
        // already been (or will be) freed by it — touching it further
        // here would be a use-after-free.
        return;
      }
      // CSS_PX_PER_MM screen px per document mm, origin (0,0): this
      // slice has no pan/zoom UI yet (specs/path-node-editing/
      // adrs.md's ViewTransform decision), so this is the whole view
      // transform, picked to keep acceptance criterion 6's 0.25mm
      // stroke and the node/handle glyphs actually visible on a real
      // screen rather than sub-pixel.
      session.set_view(CSS_PX_PER_MM, 0.0, 0.0);
      syncFromSession();
    },
    [syncFromSession],
  );

  useEffect(() => {
    let cancelled = false;

    createSession().then((session) => {
      if (cancelled) {
        session.free();
        return;
      }
      void attachSession(session);
    });

    const canvas = canvasRef.current;
    let resizeObserver: ResizeObserver | null = null;
    if (canvas) {
      resizeObserver = new ResizeObserver((entries) => {
        const entry = entries[0];
        if (!entry) {
          return;
        }
        const nextWidth = Math.max(1, Math.round(entry.contentRect.width));
        const nextHeight = Math.max(1, Math.round(entry.contentRect.height));
        canvas.width = nextWidth;
        canvas.height = nextHeight;
        sessionRef.current?.resize(nextWidth, nextHeight);
      });
      resizeObserver.observe(canvas);
    }

    // Reads `sessionRef.current` fresh every frame (rather than closing
    // over one session instance) so a New/Open swap underneath it is
    // picked up on the very next frame, with no need to restart the
    // loop itself.
    let frame = requestAnimationFrame(function renderLoop() {
      sessionRef.current?.render();
      frame = requestAnimationFrame(renderLoop);
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
      resizeObserver?.disconnect();
      sessionRef.current?.free();
      sessionRef.current = null;
    };
  }, [attachSession]);

  const newProject = useCallback(() => {
    void createSession().then(attachSession);
  }, [attachSession]);

  const openProject = useCallback(
    async (bytes: Uint8Array) => {
      const session = await openSession(bytes);
      await attachSession(session);
    },
    [attachSession],
  );

  const packProject = useCallback((appVersion: string): Uint8Array => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error("no editing session is attached yet");
    }
    return session.pack(appVersion);
  }, []);

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
      setIsHoveringPenCloseTarget(
        session?.is_hovering_pen_close_target() ?? false,
      );
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
    setIsHoveringPenCloseTarget(false);
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
    isHoveringPenCloseTarget,
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
    newProject,
    openProject,
    packProject,
  };
}
