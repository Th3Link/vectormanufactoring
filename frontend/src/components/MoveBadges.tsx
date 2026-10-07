import { ArrowLeftRight, ArrowUpDown, Plus } from "lucide-react";

import type { MoveBadgeState } from "@/hooks/useEditorSession";

/** Badge diameter, px (`docs/design-system.md`, "Modifier badge"). */
const BADGE_PX = 16;
/** The badge centre's distance left of and below the pointer hotspot, px. */
const BADGE_OFFSET_PX = 16;
/** The second badge goes this far further out, px. */
const SECOND_BADGE_SHIFT_PX = 20;
/** How far a badge drops when it mirrors to the right of the pointer and the
 * readout chip has flipped below the pointer, px. */
const CLASH_DROP_PX = 28;
/** The readout chip's height plus its 12 px offset: the pointer is closer to
 * the top edge than this, the readout flips below it. */
const READOUT_FLIP_PX = 36;

interface MoveBadgesProps {
  badges: MoveBadgeState;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * The modifier badges of a move (`specs/edit-interaction-polish/` criteria
 * 26, 27, 33; `docs/design-system.md`, "Modifier badge"): the plus badge for
 * a copy and the lock badge for the axis lock, by the pointer. They are pure
 * display: Rust decides whether each shows and which axis the lock is
 * (`Session::move_indicators`), and the hook re-reads it from window-level
 * key events and every pointer event, so a badge appears and vanishes in the
 * frame of its key. DOM, `pointer-events: none`, `aria-hidden`; no CSS cursor
 * is used because an engine applies a changed cursor only on the next mouse
 * event.
 */
export function MoveBadges({ badges, containerRef }: MoveBadgesProps) {
  const shown = [
    badges.copy ? ("copy" as const) : null,
    badges.lock ? (`lock-${badges.lock}` as const) : null,
  ].filter((badge) => badge !== null);
  if (shown.length === 0) {
    return null;
  }
  const canvasWidth = containerRef.current?.clientWidth ?? Number.POSITIVE_INFINITY;
  // Left of the pointer unless the outermost badge would leave the canvas.
  const outermost = BADGE_OFFSET_PX + (shown.length - 1) * SECOND_BADGE_SHIFT_PX + BADGE_PX / 2;
  const mirrored = badges.x - outermost < 0 && badges.x + outermost <= canvasWidth;
  const readoutBelow = badges.y < READOUT_FLIP_PX;
  const drop = mirrored && readoutBelow ? CLASH_DROP_PX : 0;
  return (
    <>
      {shown.map((kind, index) => {
        const reach = BADGE_OFFSET_PX + index * SECOND_BADGE_SHIFT_PX;
        const centreX = badges.x + (mirrored ? reach : -reach);
        const centreY = badges.y + BADGE_OFFSET_PX + drop;
        const isCopy = kind === "copy";
        return (
          <div
            key={kind}
            aria-hidden
            data-move-badge={kind}
            className="pointer-events-none absolute z-20 flex items-center justify-center rounded-full"
            style={{
              left: centreX - BADGE_PX / 2,
              top: centreY - BADGE_PX / 2,
              width: BADGE_PX,
              height: BADGE_PX,
              boxSizing: "border-box",
              background: isCopy ? "var(--editor-accent)" : "#FFFFFF",
              border: isCopy ? undefined : "1.5px solid var(--editor-accent)",
              boxShadow: "0 0 0 1px #FFFFFF",
              color: isCopy ? "#FFFFFF" : "var(--editor-accent)",
            }}
          >
            {isCopy ? (
              <Plus size={8} strokeWidth={1.5} absoluteStrokeWidth />
            ) : kind === "lock-x" ? (
              <ArrowLeftRight size={10} strokeWidth={1.5} absoluteStrokeWidth />
            ) : (
              <ArrowUpDown size={10} strokeWidth={1.5} absoluteStrokeWidth />
            )}
          </div>
        );
      })}
    </>
  );
}
