import { Tabs as TabsPrimitive } from "radix-ui";

/** Radix `Tabs`, unstyled: the parts the Properties panel's strip is built
 * from (`docs/design-system.md`, "Properties panel: tabs"). Radix gives
 * `role="tablist"`, `tab` and `tabpanel`, `aria-selected`, `aria-controls`, one
 * Tab stop with roving focus, and Home and End. */
export const Tabs = TabsPrimitive.Root;
export const TabsList = TabsPrimitive.List;
export const TabsTrigger = TabsPrimitive.Trigger;
export const TabsContent = TabsPrimitive.Content;
