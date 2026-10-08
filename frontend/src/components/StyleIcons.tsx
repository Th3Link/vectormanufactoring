import type { CapName, DashName, JoinName } from "@/hooks/useStylePanel";

const SVG_PROPS = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  "aria-hidden": true,
} as const;

/** "No stroke" / "No fill": a hollow square with a slash. */
export function NoPaintIcon() {
  return (
    <svg {...SVG_PROPS}>
      <rect x="2.5" y="2.5" width="11" height="11" />
      <line x1="2.5" y1="13.5" x2="13.5" y2="2.5" />
    </svg>
  );
}

/** "Solid stroke" / "Solid fill": a filled square. */
export function SolidPaintIcon() {
  return (
    <svg {...SVG_PROPS}>
      <rect x="2.5" y="2.5" width="11" height="11" fill="currentColor" />
    </svg>
  );
}

/** A thick corner drawn with the join under test, so the glyph shows what the
 * join does at its apex. */
export function JoinIcon({ join }: { join: JoinName }) {
  return (
    <svg {...SVG_PROPS} strokeWidth={4} strokeLinejoin={join} strokeLinecap="butt">
      <polyline points="3,14 8,4 13,14" />
    </svg>
  );
}

/** A thick segment drawn with the cap under test, with thin ticks at the true
 * endpoints so the cap's overshoot is visible. */
export function CapIcon({ cap }: { cap: CapName }) {
  return (
    <svg {...SVG_PROPS}>
      <line x1="5" y1="8" x2="11" y2="8" strokeWidth={5} strokeLinecap={cap} />
      <line x1="5" y1="2" x2="5" y2="14" strokeWidth={1} opacity={0.5} />
      <line x1="11" y1="2" x2="11" y2="14" strokeWidth={1} opacity={0.5} />
    </svg>
  );
}

/** The dash arrays of the line samples, in px at 2 px thickness
 * (`docs/design-system.md`, "Select (dash)"). */
const SAMPLE_DASHES: Record<DashName, string | undefined> = {
  solid: undefined,
  dash: "12 8",
  dot: "2 6",
  "dash-dot": "12 6 2 6",
};

/** The 64 x 8 px line sample of a dash choice. */
export function DashSample({ name }: { name: DashName | "custom" }) {
  const dash = name === "custom" ? "6 3 2 3" : SAMPLE_DASHES[name];
  return (
    <svg width="64" height="8" viewBox="0 0 64 8" aria-hidden className="shrink-0">
      <line
        x1="0"
        y1="4"
        x2="64"
        y2="4"
        stroke="var(--toolbar-icon)"
        strokeWidth="2"
        strokeDasharray={dash}
      />
    </svg>
  );
}
