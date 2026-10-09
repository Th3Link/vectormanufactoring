import { CircleAlert } from "lucide-react";

import {
  PATH_CARD_TOP_PX,
  RAIL_CARD_GAP_PX,
  TOOLS_CARD_HEIGHT_PX,
} from "@/components/railCard";
import type { ActionNotice } from "@/hooks/useActionNotice";

/** The top of the notice: level with the first button of the card that issued it. */
function noticeTop(notice: ActionNotice | null): number {
  return notice?.anchor === "path"
    ? PATH_CARD_TOP_PX
    : TOOLS_CARD_HEIGHT_PX + RAIL_CARD_GAP_PX + 4;
}

/**
 * The notice beside the rail (`docs/design-system.md`, row "Action notice"): text only, no
 * pointer events, no focus. Both live regions are in the tree from the start, so a notice is
 * announced as a change of text. A child of the rail's root (not of the scrolling card
 * column): its left edge is 12px right of the rail's right edge (`--rail-right` + 12) and its top
 * edge is level with the top of the first button of the card that issued it: Union (below the
 * tools card, the 8px gap and the Boolean card's 4px padding) or Combine (the Path card's 4px
 * padding). One status region and one alert region serve every card.
 */
export function BooleanNotice({ notice }: { notice: ActionNotice | null }) {
  return (
    <div
      className="pointer-events-none absolute z-30 flex w-max max-w-[min(360px,calc(100vw-96px))] flex-col gap-1 text-xs"
      style={{
        // The rail root sits `--rail-inset` from the viewport edge.
        left: "calc(var(--rail-right) - var(--rail-inset) + 12px)",
        top: `${noticeTop(notice)}px`,
      }}
      data-boolean-notice
    >
      <div role="status">
        {notice?.kind === "success" ? (
          <div
            className="rounded-lg p-2"
            style={{
              background: "var(--toolbar-bg)",
              color: "var(--toolbar-icon)",
              boxShadow: "var(--panel-elevation-shadow)",
            }}
          >
            {notice.text}
          </div>
        ) : null}
      </div>
      <div role="alert">
        {notice?.kind === "refusal" ? (
          <div
            className="flex items-start gap-1.5 rounded-lg bg-popover p-2"
            style={{
              color: "var(--field-invalid)",
              boxShadow: "0 0 0 1px var(--field-invalid), var(--panel-elevation-shadow)",
            }}
          >
            <CircleAlert size={12} aria-hidden="true" className="mt-0.5 shrink-0" />
            <span>{notice.text}</span>
          </div>
        ) : null}
      </div>
    </div>
  );
}
