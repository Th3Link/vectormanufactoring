import type { CSSProperties } from "react";

/**
 * The surface of a toolbox card of the tool rail (`docs/design-system.md`, row "Toolbox card"):
 * 48px wide, `--toolbar-bg`, `rounded-lg`, the panel elevation shadow, 4px padding above and
 * below, buttons in one column 4px apart. The tools card and the Boolean card both use it.
 */
export const RAIL_CARD_CLASS =
  "pointer-events-auto flex w-12 shrink-0 flex-col items-center gap-1 rounded-lg py-1";

export const RAIL_CARD_STYLE: CSSProperties = {
  background: "var(--toolbar-bg)",
  boxShadow: "var(--panel-elevation-shadow)",
};

/** Height of the tools card in px (6 buttons: 6 x 40, 5 gaps of 4, 2 x 4 padding). The notice of
 * the Boolean card is anchored level with the top of its first button, which lies below it. */
export const TOOLS_CARD_HEIGHT_PX = 268;

/** The gap between two cards of a column, in px. */
export const RAIL_CARD_GAP_PX = 8;
