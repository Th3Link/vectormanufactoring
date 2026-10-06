import { CircleAlert, Link2 } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import type { TransformEntryState } from "@/hooks/useEditorSession";
import { placeEntryChip } from "@/lib/readoutPlacement";

/** The tool rail's clearance from the canvas's left edge, px (`App.tsx`'s
 * `left-[72px]` for the contextual bars). */
const TOOL_RAIL_CLEAR_PX = 72;

/** Height of the message card plus its gap, in px. */
const MESSAGE_CARD_PX = 28;

/** The two messages of a refused field (`object-transform-refinements`
 * UX notes, "Validation"). */
const MESSAGES = {
  number: "Enter a number",
  positive: "Must be above 0",
  negative: "Must be 0 or more",
  "ratio-range": "Must be 0.01 to 0.99",
} as const;

type Reason = keyof typeof MESSAGES;

function asReason(text: string | undefined): Reason {
  return text !== undefined && text in MESSAGES ? (text as Reason) : "number";
}

interface TransformEntryChipProps {
  entry: TransformEntryState;
  /** The canvas container: the chip's coordinate space, and where focus
   * returns to. */
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** Enter: `"committed"`, `"unchanged"` or `"invalid:<field>:<reason>"`. */
  onCommit: (first: string, second: string, lastEdited: number) => string;
  onCancel: () => void;
  onLinked: (field: number, text: string) => string | undefined;
}

/**
 * The typed numeric entry chip (`specs/object-transform-refinements/
 * specification.md` criteria 18-32; `docs/design-system.md`, "Transform
 * entry chip"): a DOM text overlay next to the double-clicked handle,
 * upright whatever the object's rotation. It holds only the text, the caret
 * and the focus — validation, linking and resolution are Rust's
 * (`vecmanf-ui-core::transform_entry`). Enter commits (an invalid value
 * keeps it open, marked, with a message line); Escape, a press elsewhere
 * (which is not swallowed: the canvas processes it too), a tool switch or a
 * window blur cancels and writes nothing.
 */
export function TransformEntryChip({
  entry,
  containerRef,
  onCommit,
  onCancel,
  onLinked,
}: TransformEntryChipProps) {
  const chipRef = useRef<HTMLDivElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  const inputRefs = useRef<Array<HTMLInputElement | null>>([]);
  const messageId = useId();
  const [texts, setTexts] = useState<string[]>(() => entry.fields.map((f) => f.prefill));
  const [lastEdited, setLastEdited] = useState(0);
  const [invalid, setInvalid] = useState<{ field: number; reason: Reason } | null>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  // Focus the first editable field with its text selected, once per entry.
  useEffect(() => {
    const first = entry.fields.findIndex((f) => f.editable);
    const input = inputRefs.current[Math.max(first, 0)];
    input?.focus();
    input?.select();
    // Opened once: a repositioning re-render must not steal the selection.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Measured before paint so the chip is placed correctly on its first frame.
  useLayoutEffect(() => {
    const chip = chipRef.current;
    const row = rowRef.current;
    const container = containerRef.current;
    if (!chip || !row || !container) {
      return;
    }
    // The chip is placed by its field row (card padding included), so an
    // error message growing the card downward does not move the field.
    const next = {
      chip: { width: chip.offsetWidth, height: row.offsetHeight + 12 },
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

  const placed = placeEntryChip(entry.handle, entry.center, sizes.chip, sizes.canvas, entry.glyphReach);
  // The tool rail floats over the canvas's left edge: a chip clamped to the
  // edge, with its error message, would cover the lower end of the rail.
  const placement = { ...placed, left: Math.max(placed.left, TOOL_RAIL_CLEAR_PX) };

  // The message card goes on the side away from the handle, but flips when
  // that side has no room inside the canvas, and grows towards the canvas
  // centre horizontally so it is never clipped at a side edge.
  const awayIsAbove = placement.top + sizes.chip.height / 2 < entry.handle.y;
  const roomAbove = placement.top >= MESSAGE_CARD_PX;
  const roomBelow = sizes.canvas.height - (placement.top + sizes.chip.height) >= MESSAGE_CARD_PX;
  const messageAbove = awayIsAbove ? roomAbove || !roomBelow : !roomBelow && roomAbove;
  const messageLeft = placement.left + sizes.chip.width / 2 <= sizes.canvas.width / 2;

  const returnFocusToCanvas = () => containerRef.current?.focus();

  const selectField = (index: number) => {
    const input = inputRefs.current[index];
    input?.focus();
    input?.select();
  };

  const submit = () => {
    const outcome = onCommit(texts[0] ?? "", texts[1] ?? "", lastEdited);
    if (outcome.startsWith("invalid:")) {
      const [, field, reason] = outcome.split(":");
      setInvalid({ field: Number(field), reason: asReason(reason) });
      selectField(Number(field));
      return;
    }
    returnFocusToCanvas();
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>, index: number) => {
    if (event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      returnFocusToCanvas();
    } else if (event.key === "Tab") {
      event.preventDefault();
      const editable = entry.fields
        .map((field, i) => (field.editable ? i : -1))
        .filter((i) => i >= 0);
      if (editable.length > 1) {
        const at = editable.indexOf(index);
        const step = event.shiftKey ? -1 : 1;
        selectField(editable[(at + step + editable.length) % editable.length] ?? index);
      }
    }
  };

  const onChange = (index: number, value: string) => {
    const next = [...texts];
    next[index] = value;
    setLastEdited(index);
    setInvalid(null);
    if (entry.linked) {
      const other = onLinked(index, value);
      if (other !== undefined) {
        next[1 - index] = other;
      }
    }
    setTexts(next);
  };

  // Cancel on a blur that leaves the chip (a press elsewhere, a tool switch,
  // the window losing focus). A press on the canvas is also processed by the
  // canvas itself, so it is not swallowed.
  const onBlur = (event: React.FocusEvent<HTMLInputElement>) => {
    const next = event.relatedTarget;
    if (next instanceof Node && chipRef.current?.contains(next)) {
      return;
    }
    onCancel();
  };

  const isAngle = entry.kind === "angle";
  const isRatio = entry.kind === "inner-ratio";
  const fieldWidth = isAngle ? 80 : isRatio ? 84 : 100;
  const groupName =
    entry.kind === "angle"
      ? "Rotation"
      : entry.kind === "corner-radius"
        ? "Corner radius"
        : isRatio
          ? "Inner ratio"
          : "Size";

  return (
    <div
      ref={chipRef}
      role="group"
      aria-label={groupName}
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
      <div ref={rowRef} className="flex items-center gap-1">
        {entry.fields.map((field, index) => {
          const isInvalid = invalid?.field === index;
          return (
            <div key={field.name} className="flex items-center gap-1">
              {index === 1 && entry.linked && (
                <Link2 aria-hidden className="size-3" style={{ color: "var(--toolbar-icon)" }} />
              )}
              <div className="relative" style={{ width: fieldWidth }}>
                {field.label !== "" && (
                  <span
                    aria-hidden
                    className="pointer-events-none absolute top-1/2 left-1.5 -translate-y-1/2 text-xs"
                  >
                    {field.label}
                  </span>
                )}
                <input
                  ref={(node) => {
                    inputRefs.current[index] = node;
                  }}
                  type="text"
                  inputMode="decimal"
                  autoComplete="off"
                  spellCheck={false}
                  aria-label={field.name}
                  aria-invalid={isInvalid ? true : undefined}
                  aria-describedby={isInvalid ? messageId : undefined}
                  readOnly={!field.editable}
                  value={texts[index] ?? ""}
                  onChange={(event) => onChange(index, event.target.value)}
                  onKeyDown={(event) => onKeyDown(event, index)}
                  onBlur={onBlur}
                  className={`h-7 w-full rounded-[5px] border bg-white ${isAngle ? "pr-6" : "pr-8"} pl-5 text-right text-sm tabular-nums outline-none ${
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
                  {isAngle ? "°" : isRatio ? "" : "mm"}
                </span>
              </div>
            </div>
          );
        })}
      </div>
      {/* Always mounted so the polite live region announces a change; empty
          it has no height. The message sits outside the field row (the chip
          is anchored by the row, so it never jumps), on the side away from
          the handle: above the row when the chip is above its handle, so an
          error never covers the handle it belongs to. */}
      <div
        id={messageId}
        role="status"
        aria-live="polite"
        className={
          invalid
            ? `absolute ${messageLeft ? "left-0" : "right-0"} flex w-max min-w-full items-center gap-1 whitespace-nowrap rounded-[8px] px-1.5 py-1 text-xs ${
                messageAbove ? "bottom-full mb-1" : "top-full mt-1"
              }`
            : "absolute h-0 overflow-hidden text-xs"
        }
        style={
          invalid
            ? { background: "var(--toolbar-bg)", boxShadow: "var(--panel-elevation-shadow)" }
            : undefined
        }
      >
        {invalid && (
          <>
            <CircleAlert aria-hidden className="size-3 shrink-0" />
            <span>{MESSAGES[invalid.reason]}</span>
          </>
        )}
      </div>
    </div>
  );
}
