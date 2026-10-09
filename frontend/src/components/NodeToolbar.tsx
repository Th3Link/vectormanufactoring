import {
  Diamond,
  Merge,
  Plus,
  Spline,
  Split,
  Square,
  Trash2,
  Triangle,
  Minus as MakeLineIcon,
} from "lucide-react";
import { ContextMenu } from "radix-ui";
import type { ReactNode } from "react";

import type { NodeToolbarState } from "@/hooks/useEditorSession";
import { COMPOUND_NODES_TEXT } from "@/lib/booleanText";

export interface NodeActions {
  insertSelected: () => void;
  deleteSelected: () => void;
  convertSelected: (kind: "corner" | "symmetric" | "asymmetric") => void;
  makeLine: () => void;
  makeCurve: () => void;
  /** Join (acceptance criteria 8-11). No keyboard shortcut. */
  joinSelected: () => void;
  /** Split (acceptance criteria 12-15). No keyboard shortcut. */
  splitSelected: () => void;
}

/**
 * The three node-kind conversions (`specs/0006-path-merge-split-and-
 * node-types/specification.md`'s UX notes: "Corner/Symmetric/Asymmetric
 * becomes a 3-segment ToggleGroup... using the node glyphs themselves
 * (square/diamond/triangle) as the segment icons"). Shared between the
 * toolbar's `ToggleGroup` and the flat context menu, same reasoning as
 * `OTHER_ACTIONS` below.
 *
 * `NodeToolbarState` has no "the selection's current kind" field of its
 * own — each `can_convert_to_*` flag only says whether *some* selected
 * node could still take that conversion, not which segment matches
 * every selected node's kind right now (ambiguous anyway for a mixed-
 * kind multi-selection). So unlike `primitive-shapes`' own Polygon/Star
 * `ToggleGroup`, this one does not highlight an "active" segment; every
 * segment stays clickable, and clicking the one matching the selection's
 * kind is simply a no-op (acceptance criterion 16) — `specification.md`'s
 * own stated reason a `ToggleGroup` needs no separate disabled state for
 * its active segment applies whether or not that segment is visually
 * marked active. Left for a later pass if the `ux-engineer` wants the
 * highlight; no acceptance criterion requires it.
 */
const KIND_ACTIONS: {
  key: "corner" | "symmetric" | "asymmetric";
  label: string;
  icon: ReactNode;
  enabled: (state: NodeToolbarState) => boolean;
}[] = [
  {
    key: "corner",
    label: "Make corner",
    icon: <Square size={14} />,
    enabled: (state) => state.canConvertToCorner,
  },
  {
    key: "symmetric",
    label: "Make symmetric",
    icon: <Diamond size={14} />,
    enabled: (state) => state.canConvertToSymmetric,
  },
  {
    key: "asymmetric",
    label: "Make asymmetric",
    icon: <Triangle size={14} />,
    enabled: (state) => state.canConvertToAsymmetric,
  },
];

interface ActionDescriptor {
  key: string;
  label: string;
  icon: ReactNode;
  enabled: (state: NodeToolbarState) => boolean;
  run: (actions: NodeActions) => void;
}

/**
 * The node tool's one-shot actions (`specification.md`'s UX notes,
 * "Node-tool actions"), shared between the contextual toolbar and the
 * right-click context menu so the two never drift apart — "belt-and-
 * suspenders discoverability", same buttons either way. The three node-
 * *kind* conversions are a separate mutually-exclusive group
 * (`KIND_ACTIONS` above, rendered as a `ToggleGroup` in the toolbar),
 * not part of this list.
 */
const OTHER_ACTIONS: ActionDescriptor[] = [
  {
    key: "insert",
    label: "Insert node",
    icon: <Plus size={16} />,
    enabled: (state) => state.canInsert,
    run: (actions) => actions.insertSelected(),
  },
  {
    key: "delete",
    label: "Delete node",
    icon: <Trash2 size={16} />,
    enabled: (state) => state.canDelete,
    run: (actions) => actions.deleteSelected(),
  },
  {
    key: "make-line",
    label: "Make line",
    icon: <MakeLineIcon size={16} />,
    enabled: (state) => state.canMakeLine,
    run: (actions) => actions.makeLine(),
  },
  {
    key: "make-curve",
    label: "Make curve",
    icon: <Spline size={16} />,
    enabled: (state) => state.canMakeCurve,
    run: (actions) => actions.makeCurve(),
  },
  {
    key: "join",
    label: "Join",
    icon: <Merge size={16} />,
    enabled: (state) => state.canJoin,
    run: (actions) => actions.joinSelected(),
  },
  {
    key: "split",
    label: "Split",
    icon: <Split size={16} />,
    enabled: (state) => state.canSplit,
    run: (actions) => actions.splitSelected(),
  },
];

interface NodeToolbarProps {
  state: NodeToolbarState;
  actions: NodeActions;
}

/**
 * The contextual tool-controls bar (`specification.md`'s UX notes):
 * floating over the canvas at the top (see `App.tsx`), only while the node
 * tool is active. Buttons disable (not hide) when inapplicable. The
 * node-kind conversions are one 3-segment `ToggleGroup` (square/diamond/
 * triangle icons); Insert/Delete/Make-line/Make-curve/Join/Split are
 * individual buttons (`specs/0006-path-merge-split-and-node-types/
 * specification.md`'s UX notes).
 */
export function NodeToolbar({ state, actions }: NodeToolbarProps) {
  if (state.compoundOnly) {
    // Text only, no controls (`0016-boolean-operations` criterion 38).
    return (
      <div
        role="status"
        className="pointer-events-auto flex h-9 min-w-0 items-center rounded-lg px-3 text-sm"
        style={{
          background: "var(--toolbar-bg)",
          boxShadow: "var(--panel-elevation-shadow)",
          color: "var(--toolbar-icon)",
        }}
      >
        {COMPOUND_NODES_TEXT}
      </div>
    );
  }
  return (
    <div
      className="flex pointer-events-auto h-9 min-w-0 items-center gap-1 rounded-lg px-2"
      style={{
        background: "var(--toolbar-bg)",
        boxShadow: "var(--panel-elevation-shadow)",
      }}
    >
      <div
        role="group"
        aria-label="Node kind"
        className="flex overflow-hidden rounded-md ring-1 ring-border"
      >
        {KIND_ACTIONS.map((kind) => (
          <button
            key={kind.key}
            type="button"
            aria-label={kind.label}
            title={kind.label}
            disabled={!kind.enabled(state)}
            onClick={() => actions.convertSelected(kind.key)}
            className="flex size-7 items-center justify-center text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40"
          >
            {kind.icon}
          </button>
        ))}
      </div>

      <div className="mx-1 h-5 w-px bg-border" aria-hidden />

      {OTHER_ACTIONS.map((action) => (
        <button
          key={action.key}
          type="button"
          aria-label={action.label}
          title={action.label}
          disabled={!action.enabled(state)}
          onClick={() => action.run(actions)}
          className="flex size-7 items-center justify-center rounded-md text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40"
        >
          {action.icon}
        </button>
      ))}
    </div>
  );
}

interface NodeContextMenuProps {
  state: NodeToolbarState;
  actions: NodeActions;
  children: ReactNode;
  /** The node tool isn't active — the trigger stays mounted (so the
   * canvas it wraps never remounts and loses its attached `wgpu`
   * surface, see this component's own doc comment) but won't open. */
  disabled: boolean;
}

/**
 * The same actions on a right-click context menu over a selected node/
 * segment ("belt-and-suspenders discoverability, no memorization
 * required", same UX notes) — a flat menu (`ToggleGroup` is a toolbar-
 * only control), each item's enabled state mirroring the toolbar's.
 * Wraps the canvas: right-clicking anywhere on it opens the menu.
 *
 * Always mounted around the canvas, `disabled` rather than conditionally
 * rendered: this wrapper sits between the canvas and its parent in the
 * DOM, so swapping it in/out by tool would move the `<canvas>` to a
 * different position in the tree on every switch — React then remounts
 * it as a brand-new element, and the `WasmSession`'s attached `wgpu`
 * surface (tied to the old, now-detached element) goes dark. Keeping one
 * stable wrapper, toggling only `disabled`, is what keeps the canvas
 * itself unmounted exactly once, for the component's whole lifetime.
 */
export function NodeContextMenu({ state, actions, children, disabled }: NodeContextMenuProps) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild disabled={disabled}>
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="z-50 min-w-40 rounded-md bg-popover p-1 text-popover-foreground ring-1 ring-foreground/10">
          {KIND_ACTIONS.map((kind) => (
            <ContextMenu.Item
              key={kind.key}
              disabled={!kind.enabled(state)}
              onSelect={() => actions.convertSelected(kind.key)}
              className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none data-disabled:pointer-events-none data-disabled:opacity-40 data-highlighted:bg-muted"
            >
              {kind.icon}
              {kind.label}
            </ContextMenu.Item>
          ))}
          <ContextMenu.Separator className="my-1 h-px bg-border" />
          {OTHER_ACTIONS.map((action) => (
            <ContextMenu.Item
              key={action.key}
              disabled={!action.enabled(state)}
              onSelect={() => action.run(actions)}
              className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none data-disabled:pointer-events-none data-disabled:opacity-40 data-highlighted:bg-muted"
            >
              {action.icon}
              {action.label}
            </ContextMenu.Item>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
