// Independent tester checks of the sentences of
// `specs/0019-multi-object-transform/specification.md` criteria 54 and 55, taken
// from the criteria text, not from `src/lib/conversionText.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CONVERSION_NOTICE_MS,
  stretchHoverLine,
  stretchNotice,
} from "../src/lib/conversionText.ts";

test("criterion 54: the notice, N as plain digits", () => {
  assert.equal(stretchNotice([0, 1, 0, 0]), "Stretching turned 1 shape into a path. No undo yet.");
  assert.equal(stretchNotice([1, 1, 0, 0]), "Stretching turned 2 shapes into paths. No undo yet.");
  assert.equal(stretchNotice([0, 2500, 2500, 0]), "Stretching turned 5000 shapes into paths. No undo yet.");
  assert.equal(stretchNotice([0, 0, 0, 0]), null);
  assert.equal(stretchNotice([]), null);
  assert.equal(CONVERSION_NOTICE_MS, 6000);
});

test("criterion 55: the hover line names the kinds present, in order, plural", () => {
  assert.equal(stretchHoverLine([0, 1, 0, 0]), "Stretching turns stars into paths");
  assert.equal(
    stretchHoverLine([1, 1, 1, 0]),
    "Stretching turns polygons, stars and rotated rectangles into paths",
  );
  assert.equal(stretchHoverLine([0, 0, 3, 1]), "Stretching turns rotated rectangles and rotated ellipses into paths");
  assert.equal(stretchHoverLine([2, 0, 0, 0]), "Stretching turns polygons into paths");
  assert.equal(stretchHoverLine([0, 0, 0, 0]), null);
});
