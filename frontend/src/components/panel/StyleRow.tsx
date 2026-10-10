import type { ComponentProps } from "react";

/**
 * One 28 px row of the Style section (`docs/design-system.md`, "Rows"): the
 * label column (60 px), an 8 px gap, the control column (176 px). Extra props
 * pass through so a popover can anchor to the whole row.
 */
export function StyleRow({
  label,
  children,
  ...props
}: { label: string } & ComponentProps<"div">) {
  return (
    <div className="flex h-7 items-center gap-2" {...props}>
      <span className="w-[60px] shrink-0 text-sm text-[var(--toolbar-icon)]">{label}</span>
      <div className="flex w-[176px] items-center gap-1">{children}</div>
    </div>
  );
}
