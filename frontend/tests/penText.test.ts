// Run with `npm test`. The sentences of `src/lib/penText.ts` against the wording of
// `specs/0034-pen-path-extension/specification.md` (criteria 1, 7, 16, 18, 20).

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  NO_PEN_CUE,
  chipIsImmediate,
  chipLines,
  closeNoticeMs,
  closeNoticeText,
  closeTooltipNote,
  hasChip,
  type PenCue,
} from "../src/lib/penText.ts";

const cue = (patch: Partial<PenCue>): PenCue => ({ ...NO_PEN_CUE, ...patch });

test("the continue and join chips name the action and the Shift alternative", () => {
  assert.deepEqual(chipLines(cue({ kind: "continue" })), [
    "Continue path",
    "Shift: start a new path",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "new_path" })), [
    "Start a new path",
    "Release Shift: continue path",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "join" })), ["Join with path", "Shift: place a node"]);
  assert.deepEqual(chipLines(cue({ kind: "place_over_end" })), [
    "Place a node",
    "Release Shift: join with path",
  ]);
});

test("a join of paths with different styles has the third line", () => {
  const lines = chipLines(cue({ kind: "join", styleDiffers: true }));
  assert.equal(lines?.length, 3);
  assert.equal(lines?.[2], "The result keeps the style of the path you continue.");
});

test("the close chip reads by the default join and by Shift", () => {
  assert.deepEqual(chipLines(cue({ kind: "close", join: "sharp", asDrawn: "sharp" })), [
    "Close path with a sharp corner",
    "Shift: smooth curve",
  ]);
  assert.deepEqual(chipLines(cue({ kind: "close", join: "smooth", asDrawn: "smooth" })), [
    "Close path with a smooth curve",
    "Shift: sharp corner",
  ]);
  // Shift held over a path drawn with clicks: the join is flipped to smooth, releasing gives sharp.
  assert.deepEqual(
    chipLines(cue({ kind: "close", join: "smooth", asDrawn: "sharp", shift: true })),
    ["Close path with a smooth curve", "Release Shift: sharp corner"],
  );
  assert.deepEqual(
    chipLines(cue({ kind: "close", join: "sharp", asDrawn: "smooth", shift: true })),
    ["Close path with a sharp corner", "Release Shift: smooth curve"],
  );
});

test("only a close chip is immediate, and nothing else has a chip", () => {
  assert.equal(chipIsImmediate(cue({ kind: "close" })), true);
  assert.equal(chipIsImmediate(cue({ kind: "continue" })), false);
  assert.equal(hasChip(cue({ kind: "place" })), false);
  assert.equal(hasChip(NO_PEN_CUE), false);
  assert.equal(chipLines(NO_PEN_CUE), null);
});

test("the Close path notice (criterion 20)", () => {
  assert.equal(closeNoticeText({ closed: 3, skipped: 0 }), "Closed 3 paths. No undo yet.");
  assert.equal(closeNoticeText({ closed: 1, skipped: 0 }), "Closed 1 path. No undo yet.");
  const skipped = closeNoticeText({ closed: 2, skipped: 1 });
  assert.equal(
    skipped,
    "Closed 2 paths. 1 path has fewer than 3 nodes and was not closed. No undo yet.",
  );
  assert.equal(skipped?.length, 78, "the sentence the spec counts as 78 characters");
  assert.equal(closeNoticeMs(skipped ?? ""), 5000);
  assert.equal(closeNoticeMs("Closed 3 paths. No undo yet."), 3000);
  assert.equal(closeNoticeText({ closed: 0, skipped: 2 }), null);
});

test("the Close path tooltip note by state", () => {
  assert.equal(
    closeTooltipNote({ closable: 0, skipped: 0 }),
    "Select an open path with 3 or more nodes.",
  );
  assert.equal(closeTooltipNote({ closable: 2, skipped: 0 }), "Closes 2 open paths. No undo yet.");
  assert.equal(
    closeTooltipNote({ closable: 2, skipped: 1 }),
    "Closes 2 of 3 open paths. 1 has fewer than 3 nodes. No undo yet.",
  );
});
