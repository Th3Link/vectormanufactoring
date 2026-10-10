import { CircleAlert, ChevronDown, ChevronRight, Pencil, Star, Trash2 } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import { PageGlyph } from "@/components/panel/DocumentPresets";
import { FormatForm, OUTLINE_BUTTON, PRIMARY_BUTTON } from "@/components/panel/FormatForm";
import { PanelSwitch } from "@/components/ui/switch";
import { ToggleGroup, type ToggleOption } from "@/components/ui/toggle-group";
import { Tooltip } from "@/components/ui/tooltip";
import type { FormatGroup, FormatNotice, FormatRow, FormatsApi } from "@/hooks/useFormats";

const ICON_BUTTON =
  "flex shrink-0 items-center justify-center rounded-[5px] text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)]";

/** How long a Delete confirm ignores presses, ms (`0045` criterion 22). */
const CONFIRM_GUARD_MS = 400;

const ORIENTATION_OPTIONS: readonly ToggleOption<"portrait" | "landscape">[] = [
  {
    value: "portrait",
    label: "Opens as Portrait",
    tooltip: "A pick from this group starts taller than wide.",
    icon: (
      <span className="flex items-center gap-1 text-sm">
        <PageGlyph landscape={false} />
        Portrait
      </span>
    ),
  },
  {
    value: "landscape",
    label: "Opens as Landscape",
    tooltip: "A pick from this group starts wider than tall.",
    icon: (
      <span className="flex items-center gap-1 text-sm">
        <PageGlyph landscape />
        Landscape
      </span>
    ),
  },
];

/** A notice 4 px under the button it belongs to. */
function NoticeUnder({ notice, anchor }: { notice: FormatNotice | null; anchor: FormatNotice["anchor"] }) {
  if (!notice || notice.anchor !== anchor) {
    return null;
  }
  return notice.kind === "refusal" ? (
    <div
      role="alert"
      className="pointer-events-none absolute top-full right-0 z-30 mt-1 flex max-w-[244px] items-start gap-1 rounded-md bg-popover px-2 py-1 text-xs text-[var(--field-invalid)] ring-1 ring-[var(--field-invalid)]"
    >
      <CircleAlert size={12} aria-hidden className="mt-0.5 shrink-0" />
      {notice.text}
    </div>
  ) : (
    <>
      <p role="status" className="sr-only">
        {notice.text}
      </p>
      <p
        aria-hidden
        className="pointer-events-none absolute top-full right-0 z-30 mt-1 max-w-[244px] rounded-md bg-[var(--toolbar-bg)] px-2 py-1 text-xs text-[var(--toolbar-icon)]"
        style={{ boxShadow: "var(--panel-elevation-shadow)" }}
      >
        {notice.text}
      </p>
    </>
  );
}

interface ConfirmProps {
  prompt: string;
  onCancel: () => void;
  onDelete: () => void;
}

/** The inline confirm that replaces a row or a header (`0045` criterion 22):
 * focus on Cancel, Delete ignores presses for 400 ms, Escape cancels. */
function InlineConfirm({ prompt, onCancel, onDelete }: ConfirmProps) {
  const cancel = useRef<HTMLButtonElement>(null);
  const armed = useRef(false);
  useEffect(() => {
    cancel.current?.focus();
    const handle = window.setTimeout(() => {
      armed.current = true;
    }, CONFIRM_GUARD_MS);
    return () => window.clearTimeout(handle);
  }, []);
  return (
    <div
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          onCancel();
        }
      }}
      className="flex flex-col gap-2 rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] p-2"
    >
      <p className="text-sm text-[var(--toolbar-icon)]">{prompt}</p>
      <div className="flex gap-2">
        <button ref={cancel} type="button" onClick={onCancel} className={`${OUTLINE_BUTTON} w-[109px]`}>
          Cancel
        </button>
        <button
          type="button"
          onClick={() => {
            if (armed.current) {
              onDelete();
            }
          }}
          className={`${PRIMARY_BUTTON} w-[109px]`}
        >
          Delete
        </button>
      </div>
    </div>
  );
}

interface GroupEditProps {
  formats: FormatsApi;
  onReturnFocus: () => void;
}

/** The small form that replaces a user group's header (criterion 24). */
function GroupEditForm({ formats, onReturnFocus }: GroupEditProps) {
  const form = formats.groupForm;
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  if (!form) {
    return null;
  }
  return (
    <form
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        formats.saveGroupForm();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          formats.cancelGroupForm();
          onReturnFocus();
        }
      }}
      className="flex flex-col gap-2 rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_25%,transparent)] p-2"
    >
      <div className="flex h-7 items-center gap-2">
        <span className="w-[60px] shrink-0 text-sm text-[var(--toolbar-icon)]">Name</span>
        <div className="relative w-[158px]">
          <input
            ref={input}
            type="text"
            autoComplete="off"
            spellCheck={false}
            aria-label="Group name"
            aria-invalid={form.error ? true : undefined}
            value={form.name}
            onChange={(event) => formats.changeGroupForm({ name: event.target.value })}
            className={`style-field h-7 w-full rounded-[5px] border bg-white px-1.5 text-sm outline-none ${
              form.error
                ? "border-[var(--field-invalid)] shadow-[inset_0_0_0_2px_var(--field-invalid)]"
                : "border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] focus:border-[var(--editor-accent)]"
            }`}
          />
          {form.error && (
            <div
              role="alert"
              className="pointer-events-none absolute top-full right-0 z-30 mt-1 flex w-max max-w-[226px] items-start gap-1 rounded-md bg-popover px-2 py-1 text-xs text-[var(--field-invalid)] ring-1 ring-[var(--field-invalid)]"
            >
              <CircleAlert size={12} aria-hidden className="mt-0.5 shrink-0" />
              {form.error}
            </div>
          )}
        </div>
      </div>
      <div className="flex items-center gap-2">
        <span className="w-[60px] shrink-0 text-sm text-[var(--toolbar-icon)]">Opens as</span>
        <ToggleGroup
          label="Opens as"
          options={ORIENTATION_OPTIONS}
          value={form.orientation}
          onChange={(orientation) => formats.changeGroupForm({ orientation })}
          itemWidth={79}
          onReturnFocus={onReturnFocus}
        />
      </div>
      <div className="flex gap-2">
        <button type="submit" className={`${PRIMARY_BUTTON} w-[109px]`}>
          Save
        </button>
        <button
          type="button"
          onClick={() => {
            formats.cancelGroupForm();
            onReturnFocus();
          }}
          className={`${OUTLINE_BUTTON} w-[109px]`}
        >
          Cancel
        </button>
      </div>
    </form>
  );
}

interface RowProps {
  row: FormatRow;
  formats: FormatsApi;
  onReturnFocus: () => void;
}

function FormatRowView({ row, formats, onReturnFocus }: RowProps) {
  if (formats.confirm?.kind === "format" && formats.confirm.id === row.id) {
    return (
      <InlineConfirm prompt={row.deletePrompt} onCancel={formats.cancelDelete} onDelete={formats.doDelete} />
    );
  }
  if (formats.form?.editing === row.id) {
    return <FormatForm formats={formats} onReturnFocus={onReturnFocus} />;
  }
  const click = (action: () => void) => (event: { detail: number }) => {
    action();
    if (event.detail > 0) {
      onReturnFocus();
    }
  };
  return (
    <div
      data-rk={`fmt:${row.id}`}
      className={`flex h-7 items-center rounded-[5px] hover:bg-[var(--editor-accent-hover)] ${
        row.selected ? "bg-[var(--value-fill)] shadow-[inset_3px_0_0_var(--editor-accent)]" : ""
      }`}
    >
      {row.canStar && (
        <Tooltip
          side="left"
          content={row.favourite ? "Remove from the quick selection" : "Show in the quick selection"}
        >
          <button
            type="button"
            data-control="star"
            aria-label={row.starName}
            aria-pressed={row.favourite}
            onClick={click(() => formats.setFavourite(row.id, !row.favourite))}
            className={`${ICON_BUTTON} h-7 w-7`}
          >
            <Star size={16} aria-hidden fill={row.favourite ? "currentColor" : "none"} strokeWidth={1.5} />
          </button>
        </Tooltip>
      )}
      <button
        type="button"
        data-control="apply"
        aria-label={row.applyName}
        onClick={click(() => formats.pick(row.id))}
        className="flex h-7 min-w-0 flex-1 items-center gap-2 rounded-[5px] px-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)]"
      >
        <span className="min-w-0 flex-1 truncate text-sm text-[var(--toolbar-icon)]">{row.name}</span>
        <span className="shrink-0 text-xs tabular-nums text-[var(--panel-muted-fg)]">{row.size}</span>
      </button>
      <span className="flex w-[52px] shrink-0 justify-end">
        {row.canEdit && (
          <>
            <button
              type="button"
              data-control="edit"
              aria-label={row.editName}
              onClick={click(() => formats.openEdit(row.id))}
              className={`${ICON_BUTTON} h-6 w-6`}
            >
              <Pencil size={14} aria-hidden strokeWidth={1.5} />
            </button>
            <button
              type="button"
              data-control="delete"
              aria-label={row.deleteName}
              onClick={click(() => formats.askDelete("format", row.id))}
              className={`${ICON_BUTTON} h-6 w-6`}
            >
              <Trash2 size={14} aria-hidden strokeWidth={1.5} />
            </button>
          </>
        )}
      </span>
    </div>
  );
}

interface GroupProps {
  group: FormatGroup;
  formats: FormatsApi;
  onReturnFocus: () => void;
}

function FormatGroupView({ group, formats, onReturnFocus }: GroupProps) {
  const folded = formats.folded.has(group.id);
  const showRows = group.enabled && !folded;
  const click = (action: () => void) => (event: { detail: number }) => {
    action();
    if (event.detail > 0) {
      onReturnFocus();
    }
  };
  let header;
  if (formats.confirm?.kind === "group" && formats.confirm.id === group.id) {
    header = (
      <InlineConfirm
        prompt={group.deletePrompt}
        onCancel={formats.cancelDelete}
        onDelete={formats.doDelete}
      />
    );
  } else if (formats.groupForm?.id === group.id) {
    header = <GroupEditForm formats={formats} onReturnFocus={onReturnFocus} />;
  } else {
    header = (
      <div data-rk={`grp:${group.id}`} className="flex h-7 items-center">
        <button
          type="button"
          data-control="fold"
          aria-label={group.foldName}
          aria-expanded={showRows}
          disabled={!group.enabled}
          onClick={click(() => formats.toggleFold(group.id))}
          className={`${ICON_BUTTON} h-7 w-7 disabled:opacity-40`}
        >
          {showRows ? <ChevronDown size={12} aria-hidden /> : <ChevronRight size={12} aria-hidden />}
        </button>
        <Tooltip side="left" content={group.name}>
          <span className="min-w-0 flex-1 truncate text-sm font-semibold text-[var(--toolbar-icon)]">
            {group.name}
          </span>
        </Tooltip>
        <span className="mx-1 shrink-0 text-xs text-[var(--panel-muted-fg)]">{group.count}</span>
        {group.canEdit && (
          <>
            <button
              type="button"
              data-control="edit"
              aria-label={group.editName}
              onClick={click(() => formats.openGroupEdit(group.id))}
              className={`${ICON_BUTTON} h-6 w-6`}
            >
              <Pencil size={14} aria-hidden strokeWidth={1.5} />
            </button>
            <button
              type="button"
              data-control="delete"
              aria-label={group.deleteName}
              onClick={click(() => formats.askDelete("group", group.id))}
              className={`${ICON_BUTTON} h-6 w-6`}
            >
              <Trash2 size={14} aria-hidden strokeWidth={1.5} />
            </button>
          </>
        )}
        {group.canToggle && (
          <span className="ml-1">
            <PanelSwitch
              label="Show"
              name={group.showName}
              control="show"
              checked={group.enabled}
              onCheckedChange={(on) => formats.setGroupEnabled(group.id, on)}
              onReturnFocus={onReturnFocus}
            />
          </span>
        )}
      </div>
    );
  }
  return (
    <div className="flex flex-col">
      {header}
      {showRows &&
        group.rows.map((row) => (
          <FormatRowView key={row.id} row={row} formats={formats} onReturnFocus={onReturnFocus} />
        ))}
    </div>
  );
}

/** Roving focus over the list body: one Tab stop, Up and Down between rows,
 * Left and Right between the controls of a row (`0045` criterion 17). */
function useRovingList(root: React.RefObject<HTMLDivElement | null>) {
  const current = useRef("");
  const keyOf = (control: HTMLElement) =>
    `${control.closest<HTMLElement>("[data-rk]")?.dataset.rk ?? ""}:${control.dataset.control ?? ""}`;
  useLayoutEffect(() => {
    const element = root.current;
    if (!element) {
      return;
    }
    const controls = Array.from(element.querySelectorAll<HTMLElement>("[data-control]"));
    const active = controls.find((control) => keyOf(control) === current.current) ?? controls[0];
    for (const control of controls) {
      control.tabIndex = control === active ? 0 : -1;
    }
  });
  const onFocus = (event: React.FocusEvent) => {
    const target = event.target as HTMLElement;
    if (target.dataset.control) {
      current.current = keyOf(target);
    }
  };
  const onKeyDown = (event: React.KeyboardEvent) => {
    const target = event.target as HTMLElement;
    if (!target.dataset.control || event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }
    const element = root.current;
    const row = target.closest<HTMLElement>("[data-rk]");
    if (!element || !row) {
      return;
    }
    const rows = Array.from(element.querySelectorAll<HTMLElement>("[data-rk]"));
    const controlsOf = (r: HTMLElement) => Array.from(r.querySelectorAll<HTMLElement>("[data-control]"));
    const here = controlsOf(row);
    let next: HTMLElement | undefined;
    const rowAt = (index: number) => {
      const wanted = rows[Math.min(Math.max(index, 0), rows.length - 1)];
      const controls = controlsOf(wanted);
      return controls.find((c) => c.dataset.control === target.dataset.control) ?? controls[0];
    };
    switch (event.key) {
      case "ArrowDown":
        next = rowAt(rows.indexOf(row) + 1);
        break;
      case "ArrowUp":
        next = rowAt(rows.indexOf(row) - 1);
        break;
      case "Home":
        next = controlsOf(rows[0])[0];
        break;
      case "End":
        next = controlsOf(rows[rows.length - 1])[0];
        break;
      case "ArrowRight":
        next = here[Math.min(here.indexOf(target) + 1, here.length - 1)];
        break;
      case "ArrowLeft":
        next = here[Math.max(here.indexOf(target) - 1, 0)];
        break;
      default:
        return;
    }
    event.preventDefault();
    next?.focus();
  };
  return { onFocus, onKeyDown };
}

interface FormatListProps {
  formats: FormatsApi;
  onReturnFocus: () => void;
}

/**
 * "All formats": the disclosure under Orientation and the inline list it opens
 * (`specs/0045-document-formats-library/` criteria 12 to 17, 22 to 25). No popup:
 * the list is in the flow of the panel scroll. Everything shown, every name and
 * which buttons exist is Rust's; this holds what is open.
 */
export function FormatList({ formats, onReturnFocus }: FormatListProps) {
  const { list, expanded } = formats;
  const listId = useId();
  const root = useRef<HTMLDivElement>(null);
  const disclosure = useRef<HTMLButtonElement>(null);
  const addButton = useRef<HTMLButtonElement>(null);
  const roving = useRovingList(root);
  const [opened, setOpened] = useState(false);

  // Opening scrolls the panel so the row is at the top of the body.
  useEffect(() => {
    if (expanded && opened) {
      disclosure.current?.scrollIntoView({ block: "start" });
    }
  }, [expanded, opened]);

  const returnTo = formats.focusReturn;
  useEffect(() => {
    if (!returnTo) {
      return;
    }
    const target = returnTo.rowId
      ? root.current?.querySelector<HTMLElement>(`[data-rk="fmt:${returnTo.rowId}"] [data-control="apply"]`)
      : addButton.current;
    target?.focus();
  }, [returnTo]);

  return (
    <div className="flex flex-col">
      <button
        ref={disclosure}
        type="button"
        aria-expanded={expanded}
        aria-controls={listId}
        onClick={(event) => {
          setOpened(true);
          formats.toggleExpanded();
          if (event.detail > 0) {
            onReturnFocus();
          }
        }}
        className="flex h-7 w-full items-center gap-1 rounded-[5px] px-1 text-left text-sm text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)]"
      >
        {expanded ? <ChevronDown size={12} aria-hidden /> : <ChevronRight size={12} aria-hidden />}
        <span className="flex-1">All formats</span>
        <span className="text-xs text-[var(--panel-muted-fg)]">{list.total}</span>
      </button>
      <div id={listId} hidden={!expanded} className="mt-1 flex flex-col gap-1">
        {expanded && (
          <>
            <div
              ref={root}
              role="group"
              aria-label="Formats"
              onFocus={roving.onFocus}
              onKeyDown={roving.onKeyDown}
              className="flex flex-col gap-1"
            >
              {list.groups.map((group) => (
                <FormatGroupView
                  key={group.id}
                  group={group}
                  formats={formats}
                  onReturnFocus={onReturnFocus}
                />
              ))}
            </div>
            {list.canAdd && (
              <div className="mt-1 flex flex-col gap-2">
                <div className="relative">
                  {formats.form && formats.form.editing === null ? (
                    <FormatForm formats={formats} onReturnFocus={onReturnFocus} />
                  ) : (
                    <button
                      ref={addButton}
                      type="button"
                      onClick={formats.openAdd}
                      className={`${OUTLINE_BUTTON} w-full`}
                    >
                      Add format
                    </button>
                  )}
                  <NoticeUnder notice={formats.notice} anchor="add" />
                </div>
                <div className="relative">
                  <button type="button" onClick={formats.importFile} className={`${OUTLINE_BUTTON} w-full`}>
                    Import formats
                  </button>
                  <NoticeUnder notice={formats.notice} anchor="import" />
                </div>
                {list.canExport && (
                  <div className="relative">
                    <button type="button" onClick={formats.exportFile} className={`${OUTLINE_BUTTON} w-full`}>
                      Export my formats
                    </button>
                    <NoticeUnder notice={formats.notice} anchor="export" />
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
