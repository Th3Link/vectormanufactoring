import { CircleAlert } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import type { MoveEntryState } from "@/hooks/useEditorSession";
import { placeMoveChip } from "@/lib/readoutPlacement";

/** The tool rail's clearance from the canvas's left edge, px (`App.tsx`'s
 * `left-[72px]` for the contextual bars). */
const TOOL_RAIL_CLEAR_PX = 72;

/** Height of the message card plus its gap, in px. */
const MESSAGE_CARD_PX = 28;

/** What "Absolute" measures from (criterion 21). */
const ABSOLUTE_HINT = "Top-left corner of the object's bounds, measured from the page's top-left corner";

/** The labels and the accessible names of the two fields, per mode
 * (`edit-interaction-polish` criterion 18). */
const FIELDS = [
  { label: "X", relative: "Horizontal offset", absolute: "X position" },
  { label: "Y", relative: "Vertical offset", absolute: "Y position" },
] as const;

interface MoveEntryChipProps {
  entry: MoveEntryState;
  /** The canvas container: the chip's coordinate space, and where focus
   * returns to. */
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** Enter: `"committed"`, `"unchanged"` or `"invalid:<field>:number"`.
   * `copy` is the state of the Copy check. */
  onCommit: (first: string, second: string, absolute: boolean, copy: boolean) => string;
  onCancel: () => void;
}

/**
 * The typed move chip (`specs/0010-edit-interaction-polish/specification.md`
 * criteria 15 to 25; `docs/design-system.md`, "Move entry chip"): two fields
 * X and Y and a Relative | Absolute switch, opened on the object by a
 * double-click on the centre handle or by the key M. It holds only the text,
 * the mode, which fields the maker has edited and the focus: parsing, the
 * two readings, the tight bounds and the limits are Rust's
 * (`curvyo-ui-core::move_entry`). It opens in Relative every time, with the
 * Copy check off unless Ctrl was held at the second press of the double-click
 * (criterion 23; the key M cannot carry a Ctrl). A field
 * the maker has not edited shows the prefill of the current mode and means
 * "no change on that axis"; an edited field keeps its text across a mode
 * switch and is read in the mode current at Enter. Enter in any control
 * applies, Escape, a press elsewhere, a tool switch or a window blur cancels
 * and writes nothing.
 */
export function MoveEntryChip({ entry, containerRef, onCommit, onCancel }: MoveEntryChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const inputRefs = useRef<Array<HTMLInputElement | null>>([]);
  const switchRef = useRef<HTMLButtonElement>(null);
  const copyRef = useRef<HTMLInputElement>(null);
  const messageId = useId();
  const [absolute, setAbsolute] = useState(false);
  const [copy, setCopy] = useState(entry.copyPreset);
  const [edited, setEdited] = useState<[boolean, boolean]>([false, false]);
  const [typed, setTyped] = useState<[string, string]>(["", ""]);
  const [invalid, setInvalid] = useState<number | null>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  const prefill = (axis: 0 | 1, mode: boolean) => (mode ? entry.absolute[axis] : entry.relative[axis]);
  const textOf = (axis: 0 | 1) => (edited[axis] ? typed[axis] : prefill(axis, absolute));

  // Focus X with its text selected, once per open.
  useEffect(() => {
    inputRefs.current[0]?.focus();
    inputRefs.current[0]?.select();
  }, []);

  // Measured before paint so the chip is placed correctly on its first frame.
  useLayoutEffect(() => {
    const chip = chipRef.current;
    const body = bodyRef.current;
    const container = containerRef.current;
    if (!chip || !body || !container) {
      return;
    }
    const next = {
      chip: { width: chip.offsetWidth, height: body.offsetHeight + 12 },
      canvas: { width: container.clientWidth, height: container.clientHeight },
    };
    setSizes((previous) =>
      previous.chip.width === next.chip.width &&
      previous.chip.height === next.chip.height &&
      previous.canvas.width === next.canvas.width &&
      previous.canvas.height === next.canvas.height
        ? previous
        : next,
    );
  }, [entry, invalid, containerRef]);

  const placed = placeMoveChip(entry.center, sizes.chip, sizes.canvas);
  const placement = { ...placed, left: Math.max(placed.left, TOOL_RAIL_CLEAR_PX) };
  const roomBelow = sizes.canvas.height - (placement.top + sizes.chip.height) >= MESSAGE_CARD_PX;
  const messageLeft = placement.left + sizes.chip.width / 2 <= sizes.canvas.width / 2;

  const returnFocusToCanvas = () => containerRef.current?.focus();

  const submit = () => {
    const outcome = onCommit(textOf(0), textOf(1), absolute, copy);
    if (outcome.startsWith("invalid:")) {
      const field = Number(outcome.split(":")[1]);
      setInvalid(field);
      inputRefs.current[field]?.focus();
      inputRefs.current[field]?.select();
      return;
    }
    returnFocusToCanvas();
  };

  /** Tab order X, Y, the mode switch, the Copy check and back to X
   * (Shift+Tab reverses): the chip is a closed loop, so Tab never leaves it
   * and cancels it by a blur (`edit-interaction-polish` criteria 18, 23). The
   * two ends are handled here; the steps in between are the browser's own. */
  const loopFocus = (event: React.KeyboardEvent<HTMLElement>, first: boolean) => {
    if (event.key !== "Tab" || event.shiftKey !== first) {
      return false;
    }
    event.preventDefault();
    const target = first ? copyRef.current : inputRefs.current[0];
    target?.focus();
    if (target instanceof HTMLInputElement && target.type === "text") {
      target.select();
    }
    return true;
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      returnFocusToCanvas();
    }
  };

  const onChange = (axis: 0 | 1, value: string) => {
    setEdited((previous) => (axis === 0 ? [true, previous[1]] : [previous[0], true]));
    setTyped((previous) => (axis === 0 ? [value, previous[1]] : [previous[0], value]));
    setInvalid(null);
  };

  const onSwitchKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === " " || event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      setAbsolute((previous) => !previous);
    } else {
      onKeyDown(event);
    }
  };

  // Cancel on a blur that leaves the chip (a press elsewhere, a tool switch,
  // the window losing focus). A press on the canvas is also processed by the
  // canvas itself, so it is not swallowed.
  const onBlur = (event: React.FocusEvent<HTMLElement>) => {
    const next = event.relatedTarget;
    if (next instanceof Node && chipRef.current?.contains(next)) {
      return;
    }
    onCancel();
  };

  const segment = (active: boolean) =>
    `flex-1 px-2 text-center text-xs leading-[24px] ${active ? "rounded-[5px]" : ""}`;

  return (
    <div
      ref={chipRef}
      role="group"
      aria-label="Move"
      tabIndex={-1}
      data-transform-entry
      className="absolute z-30 rounded-[8px] p-1.5 outline-none"
      style={{
        left: placement.left,
        top: placement.top,
        background: "var(--toolbar-bg)",
        boxShadow: "var(--panel-elevation-shadow)",
        color: "var(--toolbar-icon)",
      }}
    >
      <div ref={bodyRef}>
        <div className="flex items-center gap-1">
          {FIELDS.map((field, index) => {
            const axis = index as 0 | 1;
            const isInvalid = invalid === axis;
            return (
              <div key={field.label} className="relative" style={{ width: 100 }}>
                <span
                  aria-hidden
                  className="pointer-events-none absolute top-1/2 left-1.5 -translate-y-1/2 text-xs"
                >
                  {field.label}
                </span>
                <input
                  ref={(node) => {
                    inputRefs.current[axis] = node;
                  }}
                  type="text"
                  inputMode="decimal"
                  autoComplete="off"
                  spellCheck={false}
                  aria-label={absolute ? field.absolute : field.relative}
                  aria-invalid={isInvalid ? true : undefined}
                  aria-describedby={isInvalid ? messageId : undefined}
                  value={textOf(axis)}
                  onChange={(event) => onChange(axis, event.target.value)}
                  onKeyDown={(event) => {
                    if (!(axis === 0 && loopFocus(event, true))) {
                      onKeyDown(event);
                    }
                  }}
                  onBlur={onBlur}
                  className={`h-7 w-full rounded-[5px] border bg-white pr-8 pl-5 text-right text-sm tabular-nums outline-none ${
                    isInvalid
                      ? "border-[var(--field-invalid)] shadow-[inset_0_0_0_1px_var(--field-invalid)]"
                      : "border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] focus:border-[var(--editor-accent)] focus:shadow-[inset_0_0_0_1px_var(--editor-accent)]"
                  }`}
                  style={{ color: "var(--toolbar-icon)" }}
                />
                <span
                  aria-hidden
                  className="pointer-events-none absolute top-1/2 right-1.5 -translate-y-1/2 text-xs"
                  style={{ opacity: 0.7 }}
                >
                  mm
                </span>
              </div>
            );
          })}
        </div>
        <button
          ref={switchRef}
          type="button"
          role="switch"
          aria-checked={absolute}
          aria-label="Absolute position"
          aria-description={ABSOLUTE_HINT}
          title={ABSOLUTE_HINT}
          onClick={() => setAbsolute((previous) => !previous)}
          onKeyDown={onSwitchKeyDown}
          onBlur={onBlur}
          className="mt-1.5 flex h-7 w-full cursor-pointer rounded-[7px] p-[1.5px] outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)]"
          // A 1.5 px ring as a shadow: a CSS border of 1.5 px computes to 1 px
          // at a device pixel ratio of 1, a shadow spread does not snap.
          style={{
            boxShadow: "inset 0 0 0 1.5px var(--toolbar-icon)",
            color: "var(--toolbar-icon)",
          }}
        >
          <span
            aria-hidden
            className={segment(!absolute)}
            style={
              absolute
                ? undefined
                : {
                    background: "var(--toolbar-icon-active-bg)",
                    color: "var(--toolbar-icon-active-fg)",
                  }
            }
          >
            Relative
          </span>
          <span
            aria-hidden
            className={segment(absolute)}
            style={
              absolute
                ? {
                    background: "var(--toolbar-icon-active-bg)",
                    color: "var(--toolbar-icon-active-fg)",
                  }
                : undefined
            }
          >
            Absolute
          </span>
        </button>
        <label className="mt-1.5 flex h-6 cursor-pointer items-center gap-1.5 px-1 text-xs">
          <input
            ref={copyRef}
            type="checkbox"
            checked={copy}
            aria-label="Copy"
            onChange={(event) => setCopy(event.target.checked)}
            onKeyDown={(event) => {
              if (!loopFocus(event, false)) {
                onKeyDown(event);
              }
            }}
            onBlur={onBlur}
            className="size-3.5 cursor-pointer accent-[var(--editor-accent)]"
          />
          <span aria-hidden>Copy</span>
        </label>
      </div>
      {/* Always mounted so the polite live region announces a change; empty it
          has no height. The message sits outside the body (the chip is placed
          by the body, so it never jumps), below it unless there is no room. */}
      <div
        id={messageId}
        role="status"
        aria-live="polite"
        className={
          invalid !== null
            ? `absolute ${messageLeft ? "left-0" : "right-0"} flex w-max min-w-full items-center gap-1 whitespace-nowrap rounded-[8px] px-1.5 py-1 text-xs ${
                roomBelow ? "top-full mt-1" : "bottom-full mb-1"
              }`
            : "absolute h-0 overflow-hidden text-xs"
        }
        style={
          invalid !== null
            ? { background: "var(--toolbar-bg)", boxShadow: "var(--panel-elevation-shadow)" }
            : undefined
        }
      >
        {invalid !== null && (
          <>
            <CircleAlert aria-hidden className="size-3 shrink-0" />
            <span>Enter a number</span>
          </>
        )}
      </div>
    </div>
  );
}
