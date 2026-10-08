import { Popover as PopoverPrimitive } from "radix-ui";
import type { ComponentProps } from "react";

export const Popover = PopoverPrimitive.Root;
export const PopoverTrigger = PopoverPrimitive.Trigger;
export const PopoverAnchor = PopoverPrimitive.Anchor;

/**
 * The colour popover's shell (`docs/design-system.md`, "Colour popover"):
 * non-modal, 232 px wide, 12 px padding, opens to the left of the panel so it
 * never covers the row it came from, rendered in a portal above everything.
 */
export function PopoverContent({
  className = "",
  children,
  ...props
}: ComponentProps<typeof PopoverPrimitive.Content>) {
  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        side="left"
        align="start"
        sideOffset={8}
        collisionPadding={8}
        className={`z-[70] w-[232px] rounded-lg bg-popover p-3 text-popover-foreground outline-none ${className}`}
        style={{ boxShadow: "var(--panel-elevation-shadow)" }}
        {...props}
      >
        {children}
      </PopoverPrimitive.Content>
    </PopoverPrimitive.Portal>
  );
}
