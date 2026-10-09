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

/** The dash arrays of the preset buttons' line samples, in px at 2 px
 * thickness over 34 px (`specs/0017-style-panel-rework` UX notes, section 6):
 * Dash 8 on / 5 off (three dashes), Dot 2 / 6 (five dots), Dash-dot 9 / 4 / 2 / 4
 * (two dashes, two dots). */
const SAMPLE_DASHES: Record<DashName, string | undefined> = {
  solid: undefined,
  dash: "8 5",
  dot: "2 6",
  "dash-dot": "9 4 2 4",
};

/** The 34 x 8 px line sample of a dash preset, in the colour of the text, so a
 * pressed button shows it in the pressed colour. */
export function DashSample({ name }: { name: DashName }) {
  return (
    <svg width="34" height="8" viewBox="0 0 34 8" aria-hidden className="shrink-0">
      <line
        x1="0"
        y1="4"
        x2="34"
        y2="4"
        stroke="currentColor"
        strokeWidth="2"
        strokeDasharray={SAMPLE_DASHES[name]}
      />
    </svg>
  );
}
