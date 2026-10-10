// Run with `npm test`. The sentences of `src/lib/pathText.ts` against the wording of
// `specs/0035-combine-and-break-apart/specification.md` (criteria 2, 7, 9 to 11, 16, 17).

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  breakApartRefusalText,
  breakApartSuccessText,
  combineOutlinesObjects,
  combineRefusalText,
  combineSuccessText,
  commandDimmed,
  commandName,
  commandNote,
  commandRule,
  noticeMs,
  PATH_COMMANDS,
  type BreakApartResult,
  type CombineResult,
  type PathAvailabilityState,
} from "../src/lib/pathText.ts";

const available = (patch: Partial<PathAvailabilityState>): PathAvailabilityState => ({
  needsTwo: false,
  open: 0,
  of: 3,
  canBreakApart: true,
  ...patch,
});

const combined = (patch: Partial<CombineResult>): CombineResult => ({
  kind: "applied",
  count: 4,
  of: 4,
  holes: 3,
  stylesDiffer: false,
  ...patch,
});

const broken = (patch: Partial<BreakApartResult>): BreakApartResult => ({
  kind: "applied",
  compounds: 1,
  pieces: 3,
  withHoles: false,
  leftAlone: 0,
  ...patch,
});

test("names and rules are those of criterion 2, and no rule wraps", () => {
  assert.deepEqual(PATH_COMMANDS.map(commandName), ["Combine", "Break apart"]);
  assert.equal(commandRule("combine"), "One object. Inner shapes become holes.");
  assert.equal(commandRule("break_apart"), "Splits a compound path into its pieces.");
  for (const command of PATH_COMMANDS) {
    assert.ok(commandRule(command).length <= 45, command);
  }
});

test("the Combine note: fewer than two, another tool, an open path, else the effect", () => {
  const none = available({ needsTwo: true, of: 0 });
  assert.equal(commandNote("combine", none, true), "Select two or more closed shapes.");
  assert.equal(
    commandNote("combine", none, false),
    "Select two or more closed shapes. Use the Select tool.",
  );
  assert.equal(
    commandNote("combine", available({ open: 1 }), true),
    "Needs closed paths: 1 of 3 selected is open.",
  );
  assert.equal(
    commandNote("combine", available({ open: 2 }), true),
    "Needs closed paths: 2 of 3 selected are open.",
  );
  assert.equal(
    commandNote("combine", available({}), true),
    "Replaces the selection. No undo yet.",
  );
});

test("the Break apart note: no compound path, another tool, else the effect", () => {
  const none = available({ canBreakApart: false });
  assert.equal(commandNote("break_apart", none, true), "Select a compound path.");
  assert.equal(
    commandNote("break_apart", none, false),
    "Select a compound path. Use the Select tool.",
  );
  assert.equal(
    commandNote("break_apart", available({}), true),
    "Holes stay with their piece. Replaces the selection. No undo yet.",
  );
});

test("dimming follows the session: an open path or touching shapes do not dim Combine", () => {
  assert.equal(commandDimmed("combine", available({ needsTwo: true })), true);
  assert.equal(commandDimmed("combine", available({ open: 1 })), false);
  assert.equal(commandDimmed("break_apart", available({ canBreakApart: false })), true);
  assert.equal(commandDimmed("break_apart", available({})), false);
});

test("the Combine notice counts the objects and the holes", () => {
  assert.equal(
    combineSuccessText(combined({})),
    "Combine: 4 objects became 1 compound path with 3 holes. No undo yet.",
  );
  assert.equal(
    combineSuccessText(combined({ holes: 1 })),
    "Combine: 4 objects became 1 compound path with 1 hole. No undo yet.",
  );
  assert.equal(
    combineSuccessText(combined({ holes: 0, count: 2 })),
    "Combine: 2 objects became 1 compound path. No undo yet.",
  );
  assert.equal(combineSuccessText(combined({ kind: "touching" })), null);
});

test("the Combine notice names the style that was used when the styles differed", () => {
  assert.equal(
    combineSuccessText(combined({ stylesDiffer: true })),
    "Combine: 4 objects became 1 compound path with 3 holes. It uses the style of the lowest object. No undo yet.",
  );
});

test("a notice over 70 characters lasts 5 seconds, a shorter one the default 3", () => {
  assert.equal(noticeMs(combineSuccessText(combined({ holes: 0, count: 2 })) ?? ""), undefined);
  assert.equal(noticeMs(combineSuccessText(combined({ stylesDiffer: true })) ?? ""), 5000);
  assert.equal(noticeMs("x".repeat(70)), undefined);
  assert.equal(noticeMs("x".repeat(71)), 5000);
});

test("the Combine refusals are the sentences of criteria 9 to 11", () => {
  assert.equal(
    combineRefusalText(combined({ kind: "open_paths", count: 1, of: 3 })),
    "Combine needs closed paths. 1 of 3 selected objects is open. Nothing was changed.",
  );
  assert.equal(
    combineRefusalText(combined({ kind: "touching", count: 2, of: 3 })),
    "Combine needs shapes that do not touch. 2 of 3 selected objects touch each other. Use Union to merge overlapping shapes. Nothing was changed.",
  );
  assert.equal(
    combineRefusalText(combined({ kind: "self_touching", count: 1, of: 3 })),
    "Combine needs shapes that do not cross themselves. 1 of 3 selected objects does. Nothing was changed.",
  );
  assert.equal(
    combineRefusalText(combined({ kind: "self_touching", count: 2, of: 3 })),
    "Combine needs shapes that do not cross themselves. 2 of 3 selected objects do. Nothing was changed.",
  );
  assert.equal(
    combineRefusalText(combined({ kind: "no_area", count: 1, of: 2 })),
    "Combine needs shapes that enclose an area. 1 of 2 selected objects has no area. Nothing was changed.",
  );
  assert.equal(
    combineRefusalText(combined({ kind: "out_of_range", count: 2, of: 3 })),
    "Combine works only within 10 km of the point 0, 0. 2 of 3 selected objects reach further. Nothing was changed.",
  );
  assert.equal(combineRefusalText(combined({ kind: "applied" })), null);
  assert.equal(combineRefusalText(combined({ kind: "needs_two" })), null);
});

test("every refusal of Combine outlines its offenders, a success none", () => {
  for (const kind of ["open_paths", "no_area", "out_of_range", "touching", "self_touching"]) {
    assert.equal(combineOutlinesObjects(combined({ kind })), true, kind);
  }
  assert.equal(combineOutlinesObjects(combined({ kind: "applied" })), false);
  assert.equal(combineOutlinesObjects(combined({ kind: "needs_two" })), false);
});

test("the Break apart notice counts the pieces and mentions holes and compounds left alone", () => {
  assert.equal(
    breakApartSuccessText(broken({})),
    "Break apart: 1 compound path became 3 objects. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText(broken({ compounds: 2, pieces: 5 })),
    "Break apart: 2 compound paths became 5 objects. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText(broken({ withHoles: true })),
    "Break apart: 1 compound path became 3 objects. Holes stayed with their piece. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText(broken({ leftAlone: 1 })),
    "Break apart: 1 compound path became 3 objects. 1 compound path is one piece and was left as it is. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText(broken({ withHoles: true, leftAlone: 2 })),
    "Break apart: 1 compound path became 3 objects. Holes stayed with their piece. 2 compound paths are one piece each and were left as they are. No undo yet.",
  );
  assert.equal(breakApartSuccessText(broken({ kind: "one_piece" })), null);
});

test("the Break apart refusals are the sentences of criterion 16", () => {
  assert.equal(
    breakApartRefusalText(broken({ kind: "no_compound" })),
    "Break apart needs a compound path. Nothing was changed.",
  );
  assert.equal(
    breakApartRefusalText(broken({ kind: "one_piece", compounds: 1 })),
    "Nothing to break apart. The compound path is one piece with its holes.",
  );
  assert.equal(
    breakApartRefusalText(broken({ kind: "one_piece", compounds: 2 })),
    "Nothing to break apart. The 2 selected compound paths are one piece each, with their holes.",
  );
  assert.equal(breakApartRefusalText(broken({ kind: "applied" })), null);
});
