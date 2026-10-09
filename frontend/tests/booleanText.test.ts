// Run with `npm test`. The sentences of `src/lib/booleanText.ts`, against the
// wording of `specs/0016-boolean-operations/specification.md` (criteria 1, 2,
// 15 to 17, 29).

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  BOOLEAN_OPS,
  COMPOUND_NODES_TEXT,
  operationName,
  operationRule,
  refusalOutlinesObjects,
  refusalText,
  successText,
  tooltipNote,
  type BooleanResult,
} from "../src/lib/booleanText.ts";

const result = (kind: string, count = 0, of = 0, compound = false): BooleanResult => ({
  kind,
  count,
  of,
  compound,
});

test("the five operations come in the rail order with their names and rules", () => {
  assert.deepEqual(BOOLEAN_OPS.map(operationName), [
    "Union",
    "Difference",
    "Intersection",
    "Exclusion",
    "Reverse difference",
  ]);
  assert.equal(operationRule("difference"), "The lowest selected object minus the others.");
  assert.equal(operationRule("reverse_difference"), "The top selected object minus the others.");
  assert.equal(operationRule("exclusion"), "Areas covered an odd number of times.");
});

test("the tooltip note follows criteria 1 and 2", () => {
  const ready = { needsTwo: false, open: 0, of: 0 };
  assert.equal(tooltipNote(ready, true), "Replaces the selection. No undo yet.");
  assert.equal(
    tooltipNote({ needsTwo: false, open: 2, of: 3 }, true),
    "Needs closed paths: 2 of 3 selected are open.",
  );
  assert.equal(
    tooltipNote({ needsTwo: false, open: 1, of: 3 }, true),
    "Needs closed paths: 1 of 3 selected is open.",
  );
  const dimmed = { needsTwo: true, open: 0, of: 0 };
  assert.equal(tooltipNote(dimmed, true), "Select two or more closed objects.");
  assert.equal(tooltipNote(dimmed, false), "Select two or more closed objects. Use the Select tool.");
});

test("a success names the operation and the counts (criterion 29)", () => {
  assert.equal(
    successText("union", result("applied", 3, 3, false)),
    "Union: 3 objects became 1 path. No undo yet.",
  );
  assert.equal(
    successText("difference", result("applied", 2, 2, true)),
    "Difference: 2 objects became 1 compound path. No undo yet.",
  );
  assert.equal(successText("union", result("open_paths", 1, 3)), null);
});

test("open paths and objects without area are refused with their counts (criteria 15, 16)", () => {
  assert.equal(
    refusalText("union", result("open_paths", 1, 3)),
    "Union needs closed paths. 1 of 3 selected objects is open. Nothing was changed.",
  );
  assert.equal(
    refusalText("intersection", result("open_paths", 2, 3)),
    "Intersection needs closed paths. 2 of 3 selected objects are open. Nothing was changed.",
  );
  assert.equal(
    refusalText("reverse_difference", result("no_area", 1, 3)),
    "Reverse difference needs shapes that enclose an area. 1 of 3 selected objects has no area. Nothing was changed.",
  );
  assert.equal(
    refusalText("exclusion", result("no_area", 2, 2)),
    "Exclusion needs shapes that enclose an area. 2 of 2 selected objects have no area. Nothing was changed.",
  );
});

test("objects out of range are refused with the 10 km rule (criteria 15 to 17 wording)", () => {
  assert.equal(
    refusalText("union", result("out_of_range", 1, 3)),
    "Union works only within 10 km of the point 0, 0. 1 of 3 selected objects reaches further. Nothing was changed.",
  );
  assert.equal(
    refusalText("difference", result("out_of_range", 2, 3)),
    "Difference works only within 10 km of the point 0, 0. 2 of 3 selected objects reach further. Nothing was changed.",
  );
});

test("an empty result has one sentence per operation (criterion 17)", () => {
  assert.equal(
    refusalText("intersection", result("empty")),
    "Intersection is empty: the selected objects share no area. Nothing was changed.",
  );
  assert.equal(
    refusalText("difference", result("empty")),
    "Difference is empty: the lowest object is covered completely. Nothing was changed.",
  );
  assert.equal(
    refusalText("reverse_difference", result("empty")),
    "Reverse difference is empty: the top object is covered completely. Nothing was changed.",
  );
  assert.equal(
    refusalText("exclusion", result("empty")),
    "Exclusion is empty: no area is covered an odd number of times. Nothing was changed.",
  );
});

test("results that are no refusal have no refusal text, and only some outline objects", () => {
  assert.equal(refusalText("union", result("applied", 2, 2)), null);
  assert.equal(refusalText("union", result("ignored")), null);
  assert.equal(refusalText("union", result("needs_two")), null);
  assert.equal(refusalOutlinesObjects(result("open_paths", 1, 2)), true);
  assert.equal(refusalOutlinesObjects(result("no_area", 1, 2)), true);
  assert.equal(refusalOutlinesObjects(result("empty")), false);
});

test("the compound path sentence is the one of criterion 38", () => {
  assert.equal(COMPOUND_NODES_TEXT, "Nodes of compound paths cannot be edited yet.");
});
