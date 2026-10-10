import { CircleAlert } from "lucide-react";
import { useEffect, useId, useRef } from "react";

import { PageGlyph } from "@/components/panel/DocumentPresets";
import { PresetStrip } from "@/components/panel/PresetStrip";
import { scrollBodyTo } from "@/lib/panelScroll";
import { PanelSwitch } from "@/components/ui/switch";
import { ToggleGroup, type ToggleOption } from "@/components/ui/toggle-group";
import type { FormField, FormUnit, FormatGroup, FormatsApi } from "@/hooks/useFormats";

const NEW_GROUP = "__new__";

const UNIT_OPTIONS: readonly ToggleOption<FormUnit>[] = (
  [
    ["mm", "mm, millimetres"],
    ["cm", "cm, centimetres"],
    ["in", "in, inches"],
    ["px", "px, pixels at 96 per inch"],
  ] as const
).map(([value, label]) => ({
  value,
  label,
  tooltip: label.split(", ")[1],
  icon: <span className="text-sm">{value}</span>,
}));

const ORIENTATION_OPTIONS: readonly ToggleOption<"portrait" | "landscape">[] = [
  {
    value: "portrait",
    label: "Opens as Portrait",
    tooltip: "A pick from this group starts taller than wide.",
    icon: (
      <span className="flex items-center gap-1 text-xs">
        <PageGlyph landscape={false} size={14} />
        Portrait
      </span>
    ),
  },
  {
    value: "landscape",
    label: "Opens as Landscape",
    tooltip: "A pick from this group starts wider than tall.",
    icon: (
      <span className="flex items-center gap-1 text-xs">
        <PageGlyph landscape size={14} />
        Landscape
      </span>
    ),
  },
];

const FIELD_CLASS =
  "style-field h-7 w-full rounded-[5px] border bg-white px-1.5 text-sm outline-none";
const FIELD_BORDER =
  "border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] focus:border-[var(--editor-accent)] focus:shadow-[inset_0_0_0_1px_var(--editor-accent)]";
const FIELD_INVALID = "border-[var(--field-invalid)] shadow-[inset_0_0_0_2px_var(--field-invalid)]";

export const OUTLINE_BUTTON =
  "h-7 rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] bg-transparent text-sm text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)]";
export const PRIMARY_BUTTON =
  "h-7 rounded-[5px] border border-[var(--toolbar-icon-active-bg)] bg-[var(--toolbar-icon-active-bg)] text-sm text-[var(--toolbar-icon-active-fg)] outline-none hover:brightness-110 focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1";

interface TextFieldProps {
  field: FormField;
  label: string;
  rowLabel: string;
  value: string;
  suffix?: string;
  error: string | undefined;
  showChip: boolean;
  setRef: (field: FormField, element: HTMLInputElement | null) => void;
  onChange: (value: string) => void;
  onFocus: () => void;
  numeric?: boolean;
}

/** One labelled text field of the form with its validation chip. */
function TextField({
  field,
  label,
  rowLabel,
  value,
  suffix,
  error,
  showChip,
  setRef,
  onChange,
  onFocus,
  numeric = false,
}: TextFieldProps) {
  const chipId = useId();
  return (
    <div className="flex min-h-7 items-center gap-2">
      <span className="w-[60px] shrink-0 text-sm leading-4 text-[var(--toolbar-icon)]">{rowLabel}</span>
      <div className="relative w-[158px]">
        <input
          ref={(element) => setRef(field, element)}
          type="text"
          autoComplete="off"
          spellCheck={false}
          inputMode={numeric ? "decimal" : "text"}
          aria-label={label}
          aria-invalid={error ? true : undefined}
          aria-describedby={error && showChip ? chipId : undefined}
          value={value}
          onFocus={onFocus}
          onChange={(event) => onChange(event.target.value)}
          className={`${FIELD_CLASS} ${numeric ? "text-right tabular-nums" : ""} ${error ? FIELD_INVALID : FIELD_BORDER}`}
          style={{ paddingRight: suffix ? 34 : 6 }}
        />
        {suffix && (
          <span
            aria-hidden
            className="pointer-events-none absolute top-1/2 right-1.5 -translate-y-1/2 text-sm text-[var(--panel-muted-fg)]"
          >
            {suffix}
          </span>
        )}
        {error && showChip && (
          <div
            id={chipId}
            role="alert"
            className="pointer-events-none absolute top-full right-0 z-30 mt-1 flex w-max max-w-[226px] items-start gap-1 rounded-md bg-popover px-2 py-1 text-xs text-[var(--field-invalid)] ring-1 ring-[var(--field-invalid)]"
          >
            <CircleAlert size={12} aria-hidden className="mt-0.5 shrink-0" />
            {error}
          </div>
        )}
      </div>
    </div>
  );
}

interface FormatFormProps {
  formats: FormatsApi;
  onReturnFocus: () => void;
}

/**
 * The inline form that adds or edits a format (`specs/0045-document-formats-
 * library/` criteria 18, 19 and 21): Name, Width, Height, Unit, Group, a new
 * group's name and "Opens as", "Add to quick selection", and the buttons. It
 * holds typed text only; Rust checks it and names each refused field. Enter in a
 * field presses Add (or Save), Escape closes the form and drops the draft.
 */
export function FormatForm({ formats, onReturnFocus }: FormatFormProps) {
  const { form } = formats;
  const refs = useRef<Partial<Record<FormField, HTMLInputElement>>>({});
  const root = useRef<HTMLFormElement>(null);
  const headingId = useId();
  const request = form?.focusRequest ?? null;

  // The form opens in view with the keyboard in Name; a refused press moves
  // the keyboard to the first refused field.
  useEffect(() => {
    if (request) {
      refs.current[request.field]?.focus();
    }
  }, [request]);
  useEffect(() => {
    scrollBodyTo(root.current);
  }, []);

  if (!form) {
    return null;
  }
  const { values, errors, chip } = form;
  const editing = form.editing !== null;
  const groups: FormatGroup[] = formats.list.groups;
  const newGroup = values.groupId === "";
  const setRef = (field: FormField, element: HTMLInputElement | null) => {
    if (element) {
      refs.current[field] = element;
    } else {
      delete refs.current[field];
    }
  };
  const common = (field: FormField) => ({
    field,
    error: errors[field],
    showChip: chip === field,
    setRef,
    onFocus: () => formats.focusFormField(field),
  });

  return (
    <form
      ref={root}
      aria-labelledby={headingId}
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        formats.submitForm();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          formats.cancelForm(true);
        }
      }}
      className="flex w-[244px] flex-col gap-2 rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] p-2"
    >
      <h3 id={headingId} className="sr-only">
        {editing ? "Edit format" : "Add format"}
      </h3>
      <TextField
        {...common("name")}
        label="Format name"
        rowLabel="Name"
        value={values.name}
        onChange={(name) => formats.changeForm({ name })}
      />
      <TextField
        {...common("width")}
        label="Format width"
        rowLabel="Width"
        value={values.width}
        suffix={values.unit}
        numeric
        onChange={(width) => formats.changeForm({ width })}
      />
      <TextField
        {...common("height")}
        label="Format height"
        rowLabel="Height"
        value={values.height}
        suffix={values.unit}
        numeric
        onChange={(height) => formats.changeForm({ height })}
      />
      <div className="flex h-7 items-center gap-2">
        <span className="w-[60px] shrink-0 text-sm text-[var(--toolbar-icon)]">Unit</span>
        <ToggleGroup
          label="Format unit"
          options={UNIT_OPTIONS}
          value={values.unit}
          onChange={(unit) => formats.changeForm({ unit })}
          itemWidth={36}
          onReturnFocus={onReturnFocus}
        />
      </div>
      <div className="flex flex-col gap-1">
        <span id={`${headingId}-group`} className="h-4 text-xs leading-4 font-semibold text-[var(--toolbar-icon)]">
          Group
        </span>
        <PresetStrip
          labelledBy={`${headingId}-group`}
          cells={[
            ...groups.map((group) => ({
              id: group.id,
              label: `Group ${group.name}`,
              content: group.name,
              tooltip: group.name,
            })),
            { id: NEW_GROUP, label: "New group", content: "New group…", tooltip: "Make a new group" },
          ]}
          value={newGroup ? NEW_GROUP : values.groupId}
          width="w-full"
          onPress={(id) => formats.changeForm({ groupId: id === NEW_GROUP ? "" : id })}
          onReturnFocus={onReturnFocus}
        />
      </div>
      {newGroup && (
        <>
          <TextField
            {...common("group-name")}
            label="Group name"
            rowLabel="Group name"
            value={values.newGroupName}
            onChange={(newGroupName) => formats.changeForm({ newGroupName })}
          />
          <div className="flex items-center gap-2">
            <span className="w-[60px] shrink-0 text-sm text-[var(--toolbar-icon)]">Opens as</span>
            <ToggleGroup
              label="Opens as"
              options={ORIENTATION_OPTIONS}
              value={values.orientation}
              onChange={(orientation) => formats.changeForm({ orientation })}
              itemWidth={78}
              onReturnFocus={onReturnFocus}
            />
          </div>
        </>
      )}
      {!editing && (
        <PanelSwitch
          label="Add to quick selection"
          checked={values.favourite}
          onCheckedChange={(favourite) => formats.changeForm({ favourite })}
          onReturnFocus={onReturnFocus}
        />
      )}
      <div className="flex gap-2">
        <button type="submit" className={`${PRIMARY_BUTTON} w-[109px]`}>
          {editing ? "Save" : "Add"}
        </button>
        <button
          type="button"
          onClick={(event) => {
            formats.cancelForm(event.detail === 0);
            if (event.detail > 0) {
              onReturnFocus();
            }
          }}
          className={`${OUTLINE_BUTTON} w-[109px]`}
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
