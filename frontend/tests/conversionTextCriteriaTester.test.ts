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
  assert.equal(stretchNotice(1), "Stretching turned 1 shape into a path. No undo yet.");
  assert.equal(stretchNotice(2), "Stretching turned 2 shapes into paths. No undo yet.");
  assert.equal(stretchNotice(5000), "Stretching turned 5000 shapes into paths. No undo yet.");
  assert.equal(stretchNotice(0), null);
  assert.equal(CONVERSION_NOTICE_MS, 6000);
});

test("criterion 55 (as aligned 2026-10-10): the hover line counts the shapes", () => {
  assert.equal(stretchHoverLine(2), "Stretching turns 2 shapes into paths");
  assert.equal(stretchHoverLine(1), "Stretching turns 1 shape into a path");
  assert.equal(stretchHoverLine(0), null);
});
