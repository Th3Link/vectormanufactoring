// Independent check of the sentences in `specs/0016-boolean-operations/specification.md`
// (criteria 1, 2, 15 to 17, 29, 38), written from the specification before reading the
// implementation. Every expected string is copied from the specification, not from the code.

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
} from "../src/lib/booleanText.ts";

const NAMES = ["Union", "Difference", "Intersection", "Exclusion", "Reverse difference"];

test("criterion 1: five operations in the rail order with exact names", () => {
  assert.deepEqual(BOOLEAN_OPS.map(operationName), NAMES);
});

test("criterion 2: rule lines", () => {
  assert.equal(operationRule("difference"), "The lowest selected object minus the others.");
  assert.equal(operationRule("union"), "Everything covered by any selected object.");
  assert.equal(operationRule("intersection"), "Only what every selected object covers.");
  assert.equal(operationRule("exclusion"), "Areas covered an odd number of times.");
  assert.equal(operationRule("reverse_difference"), "The top selected object minus the others.");
  for (const op of BOOLEAN_OPS) {
    assert.doesNotMatch(operationRule(op), /ctrl|shift|cmd|\+/i, "no shortcut text");
  }
});

test("criteria 1 and 2: tooltip notes", () => {
  const dim = { needsTwo: true, open: 0, of: 0 };
  assert.equal(tooltipNote(dim, true), "Select two or more closed objects.");
  assert.equal(tooltipNote(dim, false), "Select two or more closed objects. Use the Select tool.");
  assert.equal(
    tooltipNote({ needsTwo: false, open: 2, of: 3 }, true),
    "Needs closed paths: 2 of 3 selected are open.",
  );
  assert.equal(
    tooltipNote({ needsTwo: false, open: 0, of: 3 }, true),
    "Replaces the selection. No undo yet.",
  );
  // another tool wins over an open path
  assert.match(tooltipNote({ needsTwo: true, open: 1, of: 2 }, false), /Use the Select tool\.$/);
});

test("criterion 15: open path refusal, singular and plural", () => {
  assert.equal(
    refusalText("union", { kind: "open_paths", count: 1, of: 3, compound: false }),
    "Union needs closed paths. 1 of 3 selected objects is open. Nothing was changed.",
  );
  assert.equal(
    refusalText("reverse_difference", { kind: "open_paths", count: 2, of: 3, compound: false }),
    "Reverse difference needs closed paths. 2 of 3 selected objects are open. Nothing was changed.",
  );
});

test("criterion 16: no-area refusal", () => {
  assert.equal(
    refusalText("exclusion", { kind: "no_area", count: 1, of: 3, compound: false }),
    "Exclusion needs shapes that enclose an area. 1 of 3 selected objects has no area. Nothing was changed.",
  );
});

test("criterion 17: empty refusals, each followed by Nothing was changed.", () => {
  const empty = { kind: "empty", count: 0, of: 0, compound: false };
  assert.equal(
    refusalText("intersection", empty),
    "Intersection is empty: the selected objects share no area. Nothing was changed.",
  );
  assert.equal(
    refusalText("difference", empty),
    "Difference is empty: the lowest object is covered completely. Nothing was changed.",
  );
  assert.equal(
    refusalText("reverse_difference", empty),
    "Reverse difference is empty: the top object is covered completely. Nothing was changed.",
  );
  assert.equal(
    refusalText("exclusion", empty),
    "Exclusion is empty: no area is covered an odd number of times. Nothing was changed.",
  );
});

test("criterion 29: success notices", () => {
  assert.equal(
    successText("union", { kind: "applied", count: 3, of: 3, compound: false }),
    "Union: 3 objects became 1 path. No undo yet.",
  );
  assert.equal(
    successText("difference", { kind: "applied", count: 2, of: 2, compound: true }),
    "Difference: 2 objects became 1 compound path. No undo yet.",
  );
  // one line
  for (const op of BOOLEAN_OPS) {
    const text = successText(op, { kind: "applied", count: 5, of: 5, compound: true });
    assert.ok(text !== null && !text.includes("\n"));
  }
});

test("a refusal is never a success text and vice versa; ignored results say nothing", () => {
  for (const kind of ["ignored", "needs_two", "applied"]) {
    assert.equal(refusalText("union", { kind, count: 2, of: 2, compound: false }), null, kind);
  }
  for (const kind of ["empty", "open_paths", "no_area", "ignored", "needs_two", "out_of_range"]) {
    assert.equal(successText("union", { kind, count: 2, of: 2, compound: false }), null, kind);
  }
});

test("criteria 15 and 16: which refusals outline objects", () => {
  const r = (kind: string) => refusalOutlinesObjects({ kind, count: 1, of: 2, compound: false });
  assert.equal(r("open_paths"), true);
  assert.equal(r("no_area"), true);
  assert.equal(r("empty"), false);
  assert.equal(r("applied"), false);
  assert.equal(r("needs_two"), false);
});

test("criterion 38: the compound path sentence", () => {
  assert.equal(COMPOUND_NODES_TEXT, "Nodes of compound paths cannot be edited yet.");
});

test("every refusal text ends with Nothing was changed. and has no placeholder left", () => {
  for (const op of BOOLEAN_OPS) {
    for (const kind of ["open_paths", "no_area", "empty", "out_of_range"]) {
      const text = refusalText(op, { kind, count: 2, of: 4, compound: false });
      assert.ok(text !== null, `${op}/${kind}`);
      assert.ok(text.endsWith("Nothing was changed."), text);
      assert.doesNotMatch(text, /undefined|NaN|\$\{|<Op>|<Operation>/);
    }
  }
});
