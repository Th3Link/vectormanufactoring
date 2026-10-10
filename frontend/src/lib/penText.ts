/**
 * The words of the Pen's cue and of the Close path buttons (`specs/0034-pen-path-extension/`
 * criteria 1, 7, 16, 18, 20). Rust sends codes and counts; the sentences are written here, once.
 */

/** What a Pen press at the pointer would do (`WasmSession.pen_cue`). */
export interface PenCue {
  /** `"none"`, `"place"`, `"new_path"`, `"continue"`, `"join"`, `"place_over_end"` or `"close"`. */
  kind: string;
  /** The identity of the target node: a new target restarts the rest timer of the chip. */
  target: string;
  /** For a close: the join a press applies, `"sharp"` or `"smooth"`. */
  join: string;
  /** For a close: the join with no Shift. */
  asDrawn: string;
  /** Whether Shift is held (a close only). */
  shift: boolean;
  /** For a join: the continued path and the target path differ in style. */
  styleDiffers: boolean;
}

export const NO_PEN_CUE: PenCue = {
  kind: "none",
  target: "",
  join: "",
  asDrawn: "",
  shift: false,
  styleDiffers: false,
};

/** The cue's kinds that show a hint chip. */
const CHIP_KINDS = new Set(["continue", "new_path", "join", "place_over_end", "close"]);

export function hasChip(cue: PenCue): boolean {
  return CHIP_KINDS.has(cue.kind);
}

function joinWords(join: string): string {
  return join === "sharp" ? "sharp corner" : "smooth curve";
}

function otherJoinWords(join: string): string {
  return join === "sharp" ? "smooth curve" : "sharp corner";
}

/** The lines of the hint chip: the action, the Shift line, and for a join of paths that differ
 * in style a third, muted one. `null` when the cue has no chip. */
export function chipLines(cue: PenCue): string[] | null {
  switch (cue.kind) {
    case "continue":
      return ["Continue path", "Shift: start a new path"];
    case "new_path":
      return ["Start a new path", "Release Shift: continue path"];
    case "join":
      return cue.styleDiffers
        ? [
            "Join with path",
            "Shift: place a node",
            "The result keeps the style of the path you continue.",
          ]
        : ["Join with path", "Shift: place a node"];
    case "place_over_end":
      return ["Place a node", "Release Shift: join with path"];
    case "close":
      return cue.shift
        ? [
            `Close path with a ${joinWords(cue.join)}`,
            `Release Shift: ${joinWords(cue.asDrawn)}`,
          ]
        : [
            `Close path with a ${joinWords(cue.join)}`,
            `Shift: ${otherJoinWords(cue.join)}`,
          ];
    default:
      return null;
  }
}

/** Whether the chip appears at once (a close carries the choice) rather than after a rest. */
export function chipIsImmediate(cue: PenCue): boolean {
  return cue.kind === "close";
}

/** How long the pointer rests on a continue or join target before its chip appears, ms. */
export const CHIP_REST_MS = 600;

/** What a press of a Close path button did (`WasmSession.close_path`). */
export interface ClosePathResult {
  closed: number;
  skipped: number;
  /** How many of the skipped paths have three nodes whose first and last coincide. */
  sameEnds?: number;
}

/** What the Close path buttons would do (`WasmSession.close_path_state`). */
export interface ClosePathState {
  closable: number;
  skipped: number;
  /** How many of the skipped paths have three nodes whose first and last coincide. */
  sameEnds?: number;
}

function paths(count: number): string {
  return count === 1 ? "1 path" : `${count} paths`;
}

/** Why paths were skipped, in the notice: fewer than 3 nodes, or a start and end on one point. */
function skippedSentences(skipped: number, sameEnds: number): string {
  const few = skipped - sameEnds;
  const parts: string[] = [];
  if (few > 0) {
    parts.push(
      `${paths(few)} ${few === 1 ? "has" : "have"} fewer than 3 nodes and ${
        few === 1 ? "was" : "were"
      } not closed.`,
    );
  }
  if (sameEnds > 0) {
    parts.push(
      `${paths(sameEnds)} ${sameEnds === 1 ? "starts" : "start"} and ${
        sameEnds === 1 ? "ends" : "end"
      } on the same point and ${sameEnds === 1 ? "was" : "were"} not closed.`,
    );
  }
  return parts.join(" ");
}

/** The notice after a press (criterion 20), or `null` if nothing was closed. */
export function closeNoticeText(result: ClosePathResult): string | null {
  if (result.closed === 0) {
    return null;
  }
  const closed = `Closed ${paths(result.closed)}.`;
  if (result.skipped === 0) {
    return `${closed} No undo yet.`;
  }
  return `${closed} ${skippedSentences(result.skipped, result.sameEnds ?? 0)} No undo yet.`;
}

/** How long that notice stays: 3 s, or 5 s when the sentence is longer than 70 characters. */
export function closeNoticeMs(text: string): number {
  return text.length > 70 ? 5000 : 3000;
}

/** The third tooltip line of a Close path button, by state (design system, "Close path group"). */
export function closeTooltipNote(state: ClosePathState): string {
  if (state.closable === 0) {
    return "Select an open path with 3 or more nodes.";
  }
  if (state.skipped === 0) {
    return `Closes ${paths(state.closable).replace(" path", " open path")}. No undo yet.`;
  }
  const total = state.closable + state.skipped;
  const sameEnds = state.sameEnds ?? 0;
  const few = state.skipped - sameEnds;
  const reasons: string[] = [];
  if (few > 0) {
    reasons.push(`${few === 1 ? "1 has" : `${few} have`} fewer than 3 nodes.`);
  }
  if (sameEnds > 0) {
    reasons.push(
      `${sameEnds === 1 ? "1 starts and ends" : `${sameEnds} start and end`} on one point.`,
    );
  }
  return `Closes ${state.closable} of ${total} open paths. ${reasons.join(" ")} No undo yet.`;
}

/** The two buttons: the visible word, the accessible name, the join code and the tooltip. */
export const CLOSE_BUTTONS = [
  {
    join: "sharp",
    label: "Sharp",
    name: "Close path, sharp corner",
    title: "Close path with a sharp corner",
    rule: "Joins the last node to the first. The first node becomes a corner.",
  },
  {
    join: "smooth",
    label: "Smooth",
    name: "Close path, smooth curve",
    title: "Close path with a smooth curve",
    rule: "Joins the last node to the first. The first node becomes smooth.",
  },
] as const;
