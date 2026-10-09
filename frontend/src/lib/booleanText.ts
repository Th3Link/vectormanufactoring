/**
 * The words of the boolean operations (`specs/0016-boolean-operations/`
 * criteria 1, 2, 15 to 17, 29, 38): names, rules, tooltip notes and notices.
 * Rust sends codes and counts; the sentences are written here, once.
 */

/** The wire code of an operation, as `WasmSession.apply_boolean` takes it. */
export type BooleanOp =
  | "union"
  | "difference"
  | "intersection"
  | "exclusion"
  | "reverse_difference";

/** The rail order (criterion 1). */
export const BOOLEAN_OPS: readonly BooleanOp[] = [
  "union",
  "difference",
  "intersection",
  "exclusion",
  "reverse_difference",
];

/** The line of the Node bar slot and of the hint chip after a double-click
 * on a compound path (criterion 38). */
export const COMPOUND_NODES_TEXT = "Nodes of compound paths cannot be edited yet.";

/** What the rail section shows (`WasmSession.boolean_availability`). */
export interface BooleanAvailabilityState {
  /** Dimmed: fewer than two objects selected, or not the Select tool. */
  needsTwo: boolean;
  /** How many selected objects are open paths. */
  open: number;
  /** How many objects are selected (0 when `needsTwo`). */
  of: number;
}

/** What `WasmSession.apply_boolean` did (`kind` is the outcome code). */
export interface BooleanResult {
  kind: string;
  count: number;
  of: number;
  compound: boolean;
}

const NAMES: Record<BooleanOp, string> = {
  union: "Union",
  difference: "Difference",
  intersection: "Intersection",
  exclusion: "Exclusion",
  reverse_difference: "Reverse difference",
};

const RULES: Record<BooleanOp, string> = {
  union: "Everything covered by any selected object.",
  difference: "The lowest selected object minus the others.",
  intersection: "Only what every selected object covers.",
  exclusion: "Areas covered an odd number of times.",
  reverse_difference: "The top selected object minus the others.",
};

const EMPTY_TEXTS: Record<BooleanOp, string> = {
  union: "Union is empty.",
  difference: "Difference is empty: the lowest object is covered completely.",
  intersection: "Intersection is empty: the selected objects share no area.",
  exclusion: "Exclusion is empty: no area is covered an odd number of times.",
  reverse_difference: "Reverse difference is empty: the top object is covered completely.",
};

const NOTHING_CHANGED = "Nothing was changed.";

/** The operation's name; also its accessible name. */
export function operationName(op: BooleanOp): string {
  return NAMES[op];
}

/** The second tooltip line. */
export function operationRule(op: BooleanOp): string {
  return RULES[op];
}

/** The third tooltip line (criteria 1 and 2). `selectTool` is whether the
 * Select tool is active: with another tool the buttons are dimmed and the
 * note says which tool to take. */
export function tooltipNote(availability: BooleanAvailabilityState, selectTool: boolean): string {
  if (availability.needsTwo) {
    const note = "Select two or more closed objects.";
    return selectTool ? note : `${note} Use the Select tool.`;
  }
  if (availability.open > 0) {
    const verb = availability.open === 1 ? "is" : "are";
    return `Needs closed paths: ${availability.open} of ${availability.of} selected ${verb} open.`;
  }
  return "Replaces the selection. No undo yet.";
}

function plural(count: number, one: string, many: string): string {
  return count === 1 ? `1 ${one}` : `${count} ${many}`;
}

/** The notice after a success (criterion 29), or `null` if nothing was done. */
export function successText(op: BooleanOp, result: BooleanResult): string | null {
  if (result.kind !== "applied") {
    return null;
  }
  const became = result.compound ? "1 compound path" : "1 path";
  return `${NAMES[op]}: ${result.count} objects became ${became}. No undo yet.`;
}

/** The notice after a refusal (criteria 15 to 17), or `null` for a result
 * that is no refusal (`applied`, `ignored`, `needs_two`). */
export function refusalText(op: BooleanOp, result: BooleanResult): string | null {
  const name = NAMES[op];
  switch (result.kind) {
    case "open_paths": {
      const verb = result.count === 1 ? "is" : "are";
      return `${name} needs closed paths. ${result.count} of ${plural(result.of, "selected object", "selected objects")} ${verb} open. ${NOTHING_CHANGED}`;
    }
    case "no_area": {
      const verb = result.count === 1 ? "has" : "have";
      return `${name} needs shapes that enclose an area. ${result.count} of ${plural(result.of, "selected object", "selected objects")} ${verb} no area. ${NOTHING_CHANGED}`;
    }
    case "empty":
      return `${EMPTY_TEXTS[op]} ${NOTHING_CHANGED}`;
    case "out_of_range":
      return `${name} cannot handle objects this large or this far from the page. ${NOTHING_CHANGED}`;
    default:
      return null;
  }
}

/** Whether a refusal outlines objects on the canvas (criteria 15 and 16). */
export function refusalOutlinesObjects(result: BooleanResult): boolean {
  return (
    result.kind === "open_paths" || result.kind === "no_area" || result.kind === "out_of_range"
  );
}
