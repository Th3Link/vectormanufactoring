import { Check, ChevronDown } from "lucide-react";
import { Select as SelectPrimitive } from "radix-ui";
import type { ComponentProps, ReactNode } from "react";

export const Select = SelectPrimitive.Root;

const FIELD_BORDER =
  "border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)]";

interface SelectTriggerProps extends ComponentProps<typeof SelectPrimitive.Trigger> {
  /** Shown when no value is selected ("Mixed"). */
  placeholder?: string;
  children?: ReactNode;
}

/** The 28 px trigger of a panel select: white, the border of a field, the
 * current choice (or the placeholder in `--field-placeholder`) and a chevron. */
export function SelectTrigger({ placeholder, children, className = "", ...props }: SelectTriggerProps) {
  return (
    <SelectPrimitive.Trigger
      className={`flex h-7 w-full items-center justify-between gap-2 rounded-[5px] bg-white px-1.5 text-sm text-[var(--toolbar-icon)] outline-none ${FIELD_BORDER} focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)] data-[placeholder]:text-[var(--field-placeholder)] disabled:cursor-default disabled:border-transparent disabled:bg-[var(--field-disabled-bg)] disabled:text-[var(--field-disabled-fg)] ${className}`}
      {...props}
    >
      <SelectPrimitive.Value placeholder={placeholder}>{children}</SelectPrimitive.Value>
      <SelectPrimitive.Icon asChild>
        <ChevronDown size={14} aria-hidden />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
}

/** The list: in a portal over everything, items 28 px high. */
export function SelectContent({
  children,
  ...props
}: ComponentProps<typeof SelectPrimitive.Content>) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        position="popper"
        side="bottom"
        sideOffset={4}
        collisionPadding={8}
        className="z-[70] min-w-[var(--radix-select-trigger-width)] rounded-lg bg-popover p-1 text-popover-foreground"
        style={{ boxShadow: "var(--panel-elevation-shadow)" }}
        {...props}
      >
        <SelectPrimitive.Viewport>{children}</SelectPrimitive.Viewport>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
}

/** One choice: a 28 px row with the content and a check on the current one. */
export function SelectItem({
  children,
  ...props
}: ComponentProps<typeof SelectPrimitive.Item>) {
  return (
    <SelectPrimitive.Item
      className="relative flex h-7 cursor-default items-center gap-2 rounded-md px-1.5 text-sm outline-none select-none data-[disabled]:text-[var(--field-placeholder)] data-[highlighted]:bg-[var(--editor-accent-hover)]"
      {...props}
    >
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
      <SelectPrimitive.ItemIndicator className="ml-auto">
        <Check size={14} aria-hidden />
      </SelectPrimitive.ItemIndicator>
    </SelectPrimitive.Item>
  );
}
