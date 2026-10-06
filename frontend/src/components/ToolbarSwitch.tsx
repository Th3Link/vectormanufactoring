import { Switch, Tooltip } from "radix-ui";
import { useId } from "react";

export interface ToolbarSwitchProps {
  /** The visible label, left of the track. */
  label: string;
  /** What the switch does, shown as a tooltip after a short rest. */
  tooltip: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

/**
 * A Select-tool bar switch (`specs/0005-object-transform/specification.md`
 * criteria 8, 26-31 for "Scale stroke width"; `specs/unified-object-editing/`
 * criterion 23 for "Scale corner radius"; control spec in
 * `docs/design-system.md`, "Switch"): off, a resize keeps the property; on, it
 * scales with the object. A Radix `Switch` (`role="switch"`, `aria-checked`)
 * inside a `<label>`, so the whole row is the click target; Tab focuses it,
 * Space toggles. Never disabled and never dimmed: it works with nothing
 * selected too. The state is owned by `useEditorSession` (session state, off
 * per session, never saved), not here.
 */
export function ToolbarSwitch({
  label,
  tooltip,
  checked,
  onCheckedChange,
}: ToolbarSwitchProps) {
  const id = useId();
  return (
    <Tooltip.Provider>
      <Tooltip.Root delayDuration={400}>
        <Tooltip.Trigger asChild>
          <label
            htmlFor={id}
            className="flex h-7 cursor-pointer items-center gap-2 rounded-md px-2 text-sm text-[var(--toolbar-icon)] hover:bg-[var(--editor-accent-hover)] has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-[var(--editor-accent)] has-[:focus-visible]:ring-offset-1 has-[:focus-visible]:ring-offset-[var(--toolbar-bg)]"
          >
            <span>{label}</span>
            <Switch.Root
              id={id}
              checked={checked}
              onCheckedChange={onCheckedChange}
              className="inline-flex h-[18px] w-8 shrink-0 cursor-pointer items-center rounded-full border-[1.5px] border-[var(--toolbar-icon)] bg-transparent outline-none data-[state=checked]:border-[var(--toolbar-icon-active-bg)] data-[state=checked]:bg-[var(--toolbar-icon-active-bg)]"
            >
              <Switch.Thumb className="block size-3 rounded-full bg-[var(--toolbar-icon)] transition-transform duration-100 ease-out motion-reduce:transition-none data-[state=checked]:translate-x-[15px] data-[state=checked]:bg-[var(--toolbar-icon-active-fg)] data-[state=unchecked]:translate-x-[2px]" />
            </Switch.Root>
          </label>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            sideOffset={6}
            className="z-50 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground ring-1 ring-foreground/10"
          >
            {tooltip}
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  );
}
