/**
 * The words of Combine and Break apart (`specs/0035-combine-and-break-apart/` criteria 2, 7, 9 to
 * 11, 16 and 17): names, rules, tooltip notes and notices. Rust sends codes and counts; the
 * sentences are written here, once.
 */

import { NOTHING_CHANGED, plural, sharedRefusalText } from "./booleanText.ts";

/** The two commands of the Path card, in the card's order. */
export type PathCommand = "combine" | "break_apart";

export const PATH_COMMANDS: readonly PathCommand[] = ["combine", "break_apart"];

/** What the Path card shows (`WasmSession.path_availability`). */
export interface PathAvailabilityState {
  /** Combine is dimmed: fewer than two objects selected, or not the Select tool. */
  needsTwo: boolean;
  /** How many selected objects are open paths. */
  open: number;
  /** How many objects are selected (0 when `needsTwo`). */
  of: number;
  /** Break apart is enabled: the selection holds a compound path. */
  canBreakApart: boolean;
}

/** What `WasmSession.apply_combine` did (`kind` is the outcome code). */
export interface CombineResult {
  kind: string;
  count: number;
  of: number;
  holes: number;
  stylesDiffer: boolean;
}

/** What `WasmSession.apply_break_apart` did (`kind` is the outcome code). */
export interface BreakApartResult {
  kind: string;
  compounds: number;
  pieces: number;
  withHoles: boolean;
  leftAlone: number;
}

const NAMES: Record<PathCommand, string> = {
  combine: "Combine",
  break_apart: "Break apart",
};

const RULES: Record<PathCommand, string> = {
  combine: "One object. Inner shapes become holes.",
  break_apart: "Splits a compound path into its pieces.",
};

const USE_SELECT = "Use the Select tool.";

/** The command's name; also its accessible name. */
export function commandName(command: PathCommand): string {
  return NAMES[command];
}

/** The second tooltip line (under 45 characters, so it never wraps). */
export function commandRule(command: PathCommand): string {
  return RULES[command];
}

/** Whether the button is dimmed. */
export function commandDimmed(command: PathCommand, availability: PathAvailabilityState): boolean {
  return command === "combine" ? availability.needsTwo : !availability.canBreakApart;
}

/** The third tooltip line, first match wins (criterion 2). `selectTool` is whether the Select
 * tool is active: with another tool the buttons are dimmed and the note says which tool to take. */
export function commandNote(
  command: PathCommand,
  availability: PathAvailabilityState,
  selectTool: boolean,
): string {
  const withTool = (note: string) => (selectTool ? note : `${note} ${USE_SELECT}`);
  if (command === "combine") {
    if (availability.needsTwo) {
      return withTool("Select two or more closed shapes.");
    }
    if (availability.open > 0) {
      const verb = availability.open === 1 ? "is" : "are";
      return `Needs closed paths: ${availability.open} of ${availability.of} selected ${verb} open.`;
    }
    return "Replaces the selection. No undo yet.";
  }
  if (!availability.canBreakApart) {
    return withTool("Select a compound path.");
  }
  return "Holes stay with their piece. Replaces the selection. No undo yet.";
}

/** A success notice lasts 3 s, or 5 s when its sentence is longer than 70 characters (criterion
 * 7); `undefined` is the default of the kind. */
export function noticeMs(text: string): number | undefined {
  return text.length > 70 ? 5000 : undefined;
}

/** The notice after a Combine, or `null` if nothing was done (criterion 7). */
export function combineSuccessText(result: CombineResult): string | null {
  if (result.kind !== "applied") {
    return null;
  }
  const holes = result.holes === 0 ? "" : ` with ${plural(result.holes, "hole", "holes")}`;
  const style = result.stylesDiffer ? " It uses the style of the lowest object." : "";
  return `Combine: ${result.count} objects became 1 compound path${holes}.${style} No undo yet.`;
}

/** The notice after a refused Combine (criteria 9 to 11), or `null` for a result that is no
 * refusal (`applied`, `ignored`, `needs_two`). */
export function combineRefusalText(result: CombineResult): string | null {
  const objects = plural(result.of, "selected object", "selected objects");
  switch (result.kind) {
    case "touching":
      return `Combine needs shapes that do not touch. ${result.count} of ${objects} touch each other. Use Union to merge overlapping shapes. ${NOTHING_CHANGED}`;
    case "self_touching": {
      const verb = result.count === 1 ? "does" : "do";
      return `Combine needs shapes that do not cross themselves. ${result.count} of ${objects} ${verb}. ${NOTHING_CHANGED}`;
    }
    default:
      return sharedRefusalText("Combine", result);
  }
}

/** Whether a refused Combine outlines objects on the canvas (criteria 9 to 11). */
export function combineOutlinesObjects(result: CombineResult): boolean {
  return ["open_paths", "no_area", "out_of_range", "touching", "self_touching"].includes(
    result.kind,
  );
}

/** The notice after a Break apart, or `null` if nothing was done (criterion 17). */
export function breakApartSuccessText(result: BreakApartResult): string | null {
  if (result.kind !== "applied") {
    return null;
  }
  const compounds = plural(result.compounds, "compound path", "compound paths");
  let text = `Break apart: ${compounds} became ${result.pieces} objects.`;
  if (result.withHoles) {
    text += " Holes stayed with their piece.";
  }
  if (result.leftAlone === 1) {
    text += " 1 compound path is one piece and was left as it is.";
  } else if (result.leftAlone > 1) {
    text += ` ${result.leftAlone} compound paths are one piece each and were left as they are.`;
  }
  return `${text} No undo yet.`;
}

/** The notice after a refused Break apart (criterion 16), or `null` for a result that is no
 * refusal. Neither refusal outlines an object. */
export function breakApartRefusalText(result: BreakApartResult): string | null {
  switch (result.kind) {
    case "no_compound":
      return `Break apart needs a compound path. ${NOTHING_CHANGED}`;
    case "one_piece":
      return result.compounds === 1
        ? "Nothing to break apart. The compound path is one piece with its holes."
        : `Nothing to break apart. The ${result.compounds} selected compound paths are one piece each, with their holes.`;
    default:
      return null;
  }
}
