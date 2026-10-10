import { Droplet, File } from "lucide-react";
import type { ComponentType } from "react";

import { TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip } from "@/components/ui/tooltip";
import type { PanelTabView } from "@/hooks/useDocumentPanel";

/** The glyph of each tab (Lucide, 18 px, 1.5 px stroke). The History tab's
 * `History` glyph comes with `0020`. */
const GLYPHS: Record<string, ComponentType<{ size: number; strokeWidth: number; "aria-hidden": boolean }>> = {
  document: File,
  style: Droplet,
};

interface PanelTabStripProps {
  tabs: readonly PanelTabView[];
  /** After a mouse press the keyboard goes back to the canvas; a key press
   * keeps the focus on the strip so the arrows keep working. */
  onReturnFocus: () => void;
}

const TAB_CLASS =
  "flex h-7 w-9 items-center justify-center rounded-[5px] text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)] aria-selected:!bg-[var(--toolbar-icon-active-bg)] aria-selected:!text-[var(--toolbar-icon-active-fg)] disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent";

/**
 * The strip of icon tabs in the panel's header row (`specs/0043-properties-
 * tabs/` criteria 1, 2, 4, 12, 13, 21). Which tabs exist, their names, tooltips
 * and which is dimmed come from the session. The tooltip trigger shares the
 * tab's element and replaces its `data-state`, so the pressed look is keyed on
 * `aria-selected`. A dimmed tab is a disabled button (no pointer events, so
 * its tooltip sits on a wrapper), skipped by the arrows and never a Tab stop.
 */
export function PanelTabStrip({ tabs, onReturnFocus }: PanelTabStripProps) {
  return (
    <TabsList aria-label="Panel" loop={false} className="flex shrink-0 gap-1">
      {tabs.map((tab) => {
        const Glyph = GLYPHS[tab.name];
        const trigger = (
          <TabsTrigger
            value={tab.name}
            aria-label={tab.label}
            disabled={!tab.enabled}
            aria-disabled={tab.enabled ? undefined : true}
            onClick={(event) => {
              // `detail` is 0 for keyboard activation, 1 or more for a click.
              if (event.detail > 0) {
                onReturnFocus();
              }
            }}
            className={TAB_CLASS}
          >
            {Glyph && <Glyph size={18} strokeWidth={1.5} aria-hidden />}
          </TabsTrigger>
        );
        return (
          <Tooltip key={tab.name} side="left" content={tab.tooltip}>
            {tab.enabled ? trigger : <span className="inline-flex">{trigger}</span>}
          </Tooltip>
        );
      })}
    </TabsList>
  );
}
