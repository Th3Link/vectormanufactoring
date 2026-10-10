// Run with `npm test`. Independent check of the sentences quoted verbatim in
// `specs/0034-pen-path-extension` (criteria 1, 7, 16, 20) and `specs/0035-combine-and-break-apart`
// (criteria 2, 7, 9, 10, 10a, 11, 16, 17), written from the specifications.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  breakApartRefusalText,
  breakApartSuccessText,
  combineRefusalText,
  combineSuccessText,
  commandNote,
  noticeMs,
} from "../src/lib/pathText.ts";
import {
  CHIP_REST_MS,
  chipLines,
  closeNoticeText,
  closeNoticeMs,
  type PenCue,
} from "../src/lib/penText.ts";

const cue = (patch: Partial<PenCue>): PenCue => ({
  kind: "none",
  target: "t",
  join: "",
  asDrawn: "",
  shift: false,
  styleDiffers: false,
  ...patch,
});

test("0034 criterion 1: continue chip, with and without Shift", () => {
  assert.deepEqual(chipLines(cue({ kind: "continue" })), ["Continue path", "Shift: start a new path"]);
  assert.deepEqual(chipLines(cue({ kind: "new_path", shift: true })), [
    "Start a new path",
    "Release Shift: continue path",
  ]);
  assert.equal(CHIP_REST_MS, 600);
});

test("0034 criterion 7: join chip, the muted style line, and the Shift variant", () => {
  assert.deepEqual(chipLines(cue({ kind: "join" })), ["Join with path", "Shift: place a node"]);
  assert.deepEqual(chipLines(cue({ kind: "join", styleDiffers: true })), [
    "Join with path",
    "Shift: place a node",
    "The result keeps the style of the path you continue.",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "place_over_end", shift: true })), [
    "Place a node",
    "Release Shift: join with path",
  ]);
});

test("0034 criterion 16: the four close chips", () => {
  assert.deepEqual(chipLines(cue({ kind: "close", join: "sharp", asDrawn: "sharp" })), [
    "Close path with a sharp corner",
    "Shift: smooth curve",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "close", join: "smooth", asDrawn: "smooth" })), [
    "Close path with a smooth curve",
    "Shift: sharp corner",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "close", join: "smooth", asDrawn: "sharp", shift: true })), [
    "Close path with a smooth curve",
    "Release Shift: sharp corner",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "close", join: "sharp", asDrawn: "smooth", shift: true })), [
    "Close path with a sharp corner",
    "Release Shift: smooth curve",
  ]);
  assert.equal(chipLines(cue({ kind: "place" })), null);
  assert.equal(chipLines(cue({ kind: "none" })), null);
});

test("0034 criterion 20: the Close path notices and their durations", () => {
  const a = closeNoticeText({ closed: 3, skipped: 0 });
  assert.equal(a, "Closed 3 paths. No undo yet.");
  assert.equal(closeNoticeMs(a ?? ""), 3000);
  const b = closeNoticeText({ closed: 2, skipped: 1 });
  assert.equal(b, "Closed 2 paths. 1 path has fewer than 3 nodes and was not closed. No undo yet.");
  assert.equal((b ?? "").length, 78, "the skipped variant has 78 characters");
  assert.equal(closeNoticeMs(b ?? ""), 5000);
  assert.equal(closeNoticeText({ closed: 1, skipped: 0 }), "Closed 1 path. No undo yet.");
  assert.equal(closeNoticeText({ closed: 0, skipped: 0 }), null);
});

test("0035 criterion 7: the Combine notice", () => {
  const base = { kind: "applied", count: 4, of: 4, holes: 3, stylesDiffer: false };
  assert.equal(
    combineSuccessText(base),
    "Combine: 4 objects became 1 compound path with 3 holes. No undo yet.",
  );
  assert.equal(
    combineSuccessText({ ...base, holes: 1 }),
    "Combine: 4 objects became 1 compound path with 1 hole. No undo yet.",
  );
  assert.equal(
    combineSuccessText({ ...base, holes: 0 }),
    "Combine: 4 objects became 1 compound path. No undo yet.",
  );
  assert.equal(
    combineSuccessText({ ...base, stylesDiffer: true }),
    "Combine: 4 objects became 1 compound path with 3 holes. It uses the style of the lowest object. No undo yet.",
  );
  assert.equal(noticeMs("x".repeat(70)), undefined);
  assert.equal(noticeMs("x".repeat(71)), 5000);
});

test("0035 criteria 9 to 11: the Combine refusals", () => {
  const r = (kind: string, count: number, of: number) =>
    combineRefusalText({ kind, count, of, holes: 0, stylesDiffer: false });
  assert.equal(
    r("open_paths", 1, 3),
    "Combine needs closed paths. 1 of 3 selected objects is open. Nothing was changed.",
  );
  assert.equal(
    r("touching", 2, 3),
    "Combine needs shapes that do not touch. 2 of 3 selected objects touch each other. Use Union to merge overlapping shapes. Nothing was changed.",
  );
  assert.equal(
    r("self_touching", 1, 3),
    "Combine needs shapes that do not cross themselves. 1 of 3 selected objects does. Nothing was changed.",
  );
});

test("0035 criteria 16 and 17: Break apart notices and refusals", () => {
  const ok = { kind: "applied", compounds: 1, pieces: 3, withHoles: false, leftAlone: 0 };
  assert.equal(breakApartSuccessText(ok), "Break apart: 1 compound path became 3 objects. No undo yet.");
  assert.equal(
    breakApartSuccessText({ ...ok, compounds: 2, pieces: 5 }),
    "Break apart: 2 compound paths became 5 objects. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText({ ...ok, withHoles: true }),
    "Break apart: 1 compound path became 3 objects. Holes stayed with their piece. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText({ ...ok, leftAlone: 1 }),
    "Break apart: 1 compound path became 3 objects. 1 compound path is one piece and was left as it is. No undo yet.",
  );
  assert.equal(
    breakApartSuccessText({ ...ok, leftAlone: 2 }),
    "Break apart: 1 compound path became 3 objects. 2 compound paths are one piece each and were left as they are. No undo yet.",
  );
  const no = (kind: string, compounds: number) =>
    breakApartRefusalText({ kind, compounds, pieces: 0, withHoles: false, leftAlone: 0 });
  assert.equal(no("no_compound", 0), "Break apart needs a compound path. Nothing was changed.");
  assert.equal(
    no("one_piece", 1),
    "Nothing to break apart. The compound path is one piece with its holes.",
  );
  assert.equal(
    no("one_piece", 2),
    "Nothing to break apart. The 2 selected compound paths are one piece each, with their holes.",
  );
});

test("0035 criterion 2: tooltip notes, first match wins", () => {
  const st = (p: Partial<{ needsTwo: boolean; open: number; of: number; canBreakApart: boolean }>) => ({
    needsTwo: false,
    open: 0,
    of: 3,
    canBreakApart: true,
    ...p,
  });
  assert.equal(commandNote("combine", st({ needsTwo: true, of: 0 }), true), "Select two or more closed shapes.");
  assert.equal(
    commandNote("combine", st({ needsTwo: true, of: 0 }), false),
    "Select two or more closed shapes. Use the Select tool.",
  );
  assert.equal(commandNote("combine", st({ open: 1 }), true), "Needs closed paths: 1 of 3 selected is open.");
  assert.equal(commandNote("combine", st({}), true), "Replaces the selection. No undo yet.");
  assert.equal(commandNote("break_apart", st({ canBreakApart: false }), true), "Select a compound path.");
  assert.equal(
    commandNote("break_apart", st({ canBreakApart: false }), false),
    "Select a compound path. Use the Select tool.",
  );
  assert.equal(
    commandNote("break_apart", st({}), true),
    "Holes stay with their piece. Replaces the selection. No undo yet.",
  );
});
