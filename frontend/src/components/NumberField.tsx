import { CircleAlert } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";

export interface NumberFieldProps {
  /** The accessible name ("Stroke width, millimetres"). */
  label: string;
  /** The committed value as text ("0.25", "#2F6FEE", "50"); ignored when mixed. */
  shown: string;
  /** The edited objects differ: empty, with the placeholder "Mixed". */
  mixed: boolean;
  /** The fixed unit inside the right edge ("mm", "%"), or none. */
  suffix?: string;
  /** Width in px: 96 for a width, 84 for hex, 56 for an opacity. */
  width: number;
  /** Hex fields are left-aligned, the number fields right-aligned. */
  align?: "left" | "right";
  disabled: boolean;
  /** Enter or Tab: `"committed"`, `"unchanged"` or `"invalid:<code>"`. */
  onSubmit: (text: string) => string;
  /** The message of each `invalid:<code>`. */
  messages: Readonly<Record<string, string>>;
  /** After Enter or Escape: the keyboard goes back to the canvas. */
  onReturnFocus: () => void;
  /** Marks the field as the panel's first focus target when it is the first
   * enabled control. */
  firstFocus?: boolean;
  /** 12 px text and a narrower unit gutter, for the rows of the stop list. */
  compact?: boolean;
  /** Leaving the field with edited text commits it, as Enter does (the
   * Document section's size fields, criterion 15); a refused value stays in
   * the field with its message. Otherwise leaving restores. */
  commitOnBlur?: boolean;
  /** The virtual keyboard a touch device shows: `decimal` for a plain number
   * (the Document size), `text` (the default) for hex and anything with text. */
  inputMode?: "text" | "decimal";
}

/**
 * A typed field of the Style panel (`specs/0007-stroke-and-fill-styling`
 * criterion 36, `docs/design-system.md`, "Number field"). Nothing previews
 * while typing. Enter commits and returns focus to the canvas; Tab commits and
 * moves on; Escape or a press elsewhere restores the shown value and writes
 * nothing. Enter on text the maker did not touch writes nothing. A refused
 * value keeps focus, selects the text and shows a message chip below the field
 * (an overlay, so no row shifts) until the next keystroke. The rules are
 * Rust's; this holds the text, the caret and the invalid mark.
 */
export function NumberField({
  label,
  shown,
  mixed,
  suffix,
  width,
  align = "right",
  disabled,
  onSubmit,
  messages,
  onReturnFocus,
  firstFocus = false,
  compact = false,
  commitOnBlur = false,
  inputMode = "text",
}: NumberFieldProps) {
  const messageId = useId();
  const display = mixed ? "" : shown;
  const [text, setText] = useState(display);
  const [invalid, setInvalid] = useState<string | null>(null);
  const editing = useRef(false);
  const touched = useRef(false);

  // Follow the document while the maker is not typing.
  useEffect(() => {
    if (!editing.current) {
      setText(display);
      setInvalid(null);
    }
  }, [display]);

  const restore = () => {
    editing.current = false;
    touched.current = false;
    setText(display);
    setInvalid(null);
  };

  /** Commits the typed text if it was touched. `true` when focus may leave. */
  const submit = (input: HTMLInputElement): boolean => {
    if (!touched.current) {
      return true;
    }
    const outcome = onSubmit(text);
    if (outcome.startsWith("invalid:")) {
      setInvalid(outcome.slice("invalid:".length));
      input.select();
      return false;
    }
    editing.current = false;
    touched.current = false;
    return true;
  };

  const message = invalid ? (messages[invalid] ?? "Not a valid value") : null;

  return (
    <div className="relative" style={{ width }}>
      <input
        type="text"
        inputMode={inputMode}
        autoComplete="off"
        spellCheck={false}
        aria-label={label}
        aria-invalid={invalid ? true : undefined}
        aria-describedby={invalid ? messageId : undefined}
        placeholder={mixed ? "Mixed" : undefined}
        disabled={disabled}
        value={text}
        data-first-focus={firstFocus ? "" : undefined}
        onFocus={(event) => {
          editing.current = true;
          event.currentTarget.select();
        }}
        onChange={(event) => {
          touched.current = true;
          setText(event.target.value);
          setInvalid(null);
        }}
        onBlur={() => {
          if (!(commitOnBlur && touched.current)) {
            restore();
            return;
          }
          const outcome = onSubmit(text);
          editing.current = false;
          if (outcome.startsWith("invalid:")) {
            // The text stays, with its message, until the next keystroke.
            setInvalid(outcome.slice("invalid:".length));
          } else {
            // Committed or unchanged: the field shows the document's text
            // again (" 300 " on 300 mm becomes "300"), also when nothing
            // changed and so no new text arrives.
            restore();
          }
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            if (submit(event.currentTarget)) {
              event.currentTarget.blur();
              onReturnFocus();
            }
          } else if (event.key === "Tab") {
            // Tab commits and moves on; a refused value keeps focus.
            if (!submit(event.currentTarget)) {
              event.preventDefault();
            }
          } else if (event.key === "Escape") {
            event.preventDefault();
            restore();
            event.currentTarget.blur();
            onReturnFocus();
          }
        }}
        className={`style-field h-7 w-full rounded-[5px] border bg-white pl-1.5 ${compact ? "text-xs" : "text-sm"} tabular-nums outline-none disabled:border-transparent disabled:bg-[var(--field-disabled-bg)] disabled:text-[var(--field-disabled-fg)] ${
          align === "right" ? "text-right" : "text-left"
        } ${suffix && !mixed ? (suffix === "mm" || suffix === "cm" || suffix === "in" ? "pr-8" : compact ? "pr-[18px]" : "pr-6") : "pr-1.5"} ${
          invalid
            ? "border-[var(--field-invalid)] shadow-[inset_0_0_0_2px_var(--field-invalid)]"
            : "border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] focus:border-[var(--editor-accent)] focus:shadow-[inset_0_0_0_1px_var(--editor-accent)]"
        }`}
      />
      {suffix && !mixed && (
        <span
          aria-hidden
          className={`pointer-events-none absolute top-1/2 right-1.5 -translate-y-1/2 ${compact ? "text-xs" : "text-sm"} ${
            disabled ? "text-[var(--field-disabled-fg)]" : "text-[var(--panel-muted-fg)]"
          }`}
        >
          {suffix}
        </span>
      )}
      {message && (
        <div
          id={messageId}
          role="alert"
          className="pointer-events-none absolute top-full right-0 z-30 mt-1 flex w-max max-w-[220px] items-start gap-1 rounded-md bg-popover px-2 py-1 text-xs text-[var(--field-invalid)] ring-1 ring-[var(--field-invalid)]"
        >
          <CircleAlert size={12} aria-hidden className="mt-0.5 shrink-0" />
          {message}
        </div>
      )}
    </div>
  );
}
