import { Switch as SwitchPrimitive } from "radix-ui";

interface PanelSwitchProps {
  /** The visible label, 14 px at the left of the track. */
  label: string;
  /** The accessible name when it says more than the label ("Show group Slides"). */
  name?: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** After a mouse click the keyboard goes back to the canvas. */
  onReturnFocus: () => void;
  /** The roving-focus key of a list that owns this control. */
  control?: string;
}

/**
 * The panel's switch (`docs/design-system.md`, "Document formats library"):
 * the label at the left of a 32 x 18 track. Radix `Switch` is a button with
 * `role="switch"`: Space toggles and Enter does not.
 */
export function PanelSwitch({
  label,
  name,
  checked,
  onCheckedChange,
  onReturnFocus,
  control,
}: PanelSwitchProps) {
  return (
    <label className="flex items-center gap-2 text-sm text-[var(--toolbar-icon)]">
      <span aria-hidden>{label}</span>
      <SwitchPrimitive.Root
        checked={checked}
        aria-label={name ?? label}
        data-control={control}
        onCheckedChange={onCheckedChange}
        onKeyDown={(event) => {
          // Only Space toggles a switch (`0045` criterion 17).
          if (event.key === "Enter") {
            event.preventDefault();
          }
        }}
        onClick={(event) => {
          // `detail` is 0 for keyboard activation, 1 or more for a click.
          if (event.detail > 0) {
            onReturnFocus();
          }
        }}
        className="relative h-[18px] w-8 shrink-0 rounded-full border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] bg-transparent outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] data-[state=checked]:border-[var(--toolbar-icon-active-bg)] data-[state=checked]:bg-[var(--toolbar-icon-active-bg)]"
      >
        <SwitchPrimitive.Thumb className="block h-3 w-3 translate-x-[2px] rounded-full bg-[var(--toolbar-icon)] transition-none data-[state=checked]:translate-x-[16px] data-[state=checked]:bg-[var(--toolbar-icon-active-fg)]" />
      </SwitchPrimitive.Root>
    </label>
  );
}
