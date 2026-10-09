import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent } from "react";

import type { GridName } from "@/hooks/useStylePanel";

/** A press that has moved this far (px) starts a drag; less is a click. */
export const DRAG_THRESHOLD_PX = 3;

/** The factor a modifier gives the drag: Shift ten times as fast, Ctrl (Cmd on
 * macOS) ten times slower (`specs/0017-style-panel-rework` criterion 38). */
function factorOf(event: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }): number {
  if (event.shiftKey) {
    return 10;
  }
  return event.ctrlKey || event.metaKey ? 0.1 : 1;
}

/** The rounding grid that goes with a factor. */
export function gridOf(event: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }): GridName {
  const factor = factorOf(event);
  return factor === 10 ? "coarse" : factor === 0.1 ? "fine" : "normal";
}

interface Drag {
  pointerId: number;
  startX: number;
  /** The position `p` and pointer x the factor is applied from. */
  baseP: number;
  baseX: number;
  factor: number;
  lastP: number;
  lastX: number;
  dragging: boolean;
  left: number;
  width: number;
}

interface ValueDragOptions {
  /** The bar position at the press, 0 to 1. */
  bar: number;
  /** The edited objects differ: the pointer's own position is the value. */
  mixed: boolean;
  /** A tick: position `p` on the field's scale and the grid. */
  onTick: (p: number, grid: GridName) => void;
  /** A press and release with less than the threshold of movement. */
  onClick: () => void;
  /** After a drag: the keyboard goes back to the canvas. */
  onDragEnd: () => void;
}

/**
 * The pointer gesture of a value field (`specs/0017-style-panel-rework`
 * criteria 36 to 40, 44, 45): a drag starts once the pointer has moved 3 px,
 * changes the value relative to the value at the press, keeps the pointer
 * captured, clamps at the ends without re-basing there, and re-bases only when
 * a modifier changes. The mapping from `p` to a value is Rust's; this sends `p`.
 * A mixed field sets the value under the pointer. The commit, Escape and the
 * one-commit rule are `usePreviewGesture`'s, installed by the first tick.
 */
export function useValueDrag({ bar, mixed, onTick, onClick, onDragEnd }: ValueDragOptions) {
  const drag = useRef<Drag | null>(null);
  const [modifier, setModifier] = useState<GridName>("normal");

  const finish = useCallback(() => {
    drag.current = null;
    document.documentElement.removeAttribute("data-value-drag");
    setModifier("normal");
  }, []);

  // The cursor stays a resize cursor everywhere until the release; the
  // modifier words follow the keys even when the pointer holds still.
  useEffect(
    () => () => {
      document.documentElement.removeAttribute("data-value-drag");
    },
    [],
  );

  const rebase = useCallback(
    (event: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) => {
      const current = drag.current;
      if (!current?.dragging) {
        return;
      }
      const factor = factorOf(event);
      if (factor !== current.factor) {
        current.baseP = current.lastP;
        current.baseX = current.lastX;
        current.factor = factor;
        setModifier(gridOf(event));
        onTick(current.lastP, gridOf(event));
      }
    },
    [onTick],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => rebase(event);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("keyup", onKey, true);
    };
  }, [rebase]);

  const onPointerDown = (event: PointerEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    const box = event.currentTarget.getBoundingClientRect();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      baseP: bar,
      baseX: event.clientX,
      factor: factorOf(event),
      lastP: bar,
      lastX: event.clientX,
      dragging: false,
      left: box.left,
      width: box.width,
    };
  };

  const onPointerMove = (event: PointerEvent<HTMLElement>) => {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) {
      return;
    }
    if (!current.dragging) {
      if (Math.abs(event.clientX - current.startX) < DRAG_THRESHOLD_PX) {
        return;
      }
      current.dragging = true;
      document.documentElement.setAttribute("data-value-drag", "");
    }
    rebase(event);
    const raw = mixed
      ? (event.clientX - current.left) / current.width
      : current.baseP + (current.factor * (event.clientX - current.baseX)) / current.width;
    const p = Math.min(1, Math.max(0, raw));
    current.lastP = p;
    current.lastX = event.clientX;
    onTick(p, gridOf(event));
  };

  const onPointerUp = (event: PointerEvent<HTMLElement>) => {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) {
      return;
    }
    const wasDrag = current.dragging;
    finish();
    if (wasDrag) {
      onDragEnd();
    } else {
      onClick();
    }
  };

  const onPointerCancel = () => {
    finish();
  };

  return { modifier, handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel } };
}
