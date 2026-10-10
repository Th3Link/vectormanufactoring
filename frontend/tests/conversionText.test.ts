// Run with `npm test`. The sentences of `src/lib/conversionText.ts`, against the
// wording of `specs/0019-multi-object-transform/specification.md` (criteria 23, 34,
// 54, 55 and the lead's decision of 2026-10-10).

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CONVERSION_NOTICE_MS,
  stretchEntryNote,
  stretchHoverLine,
  stretchNotice,
  stretchReadoutLine,
} from "../src/lib/conversionText.ts";

test("nothing converting writes no sentence at all", () => {
  assert.equal(stretchHoverLine(0), null);
  assert.equal(stretchReadoutLine(0), null);
  assert.equal(stretchNotice(0), null);
  assert.equal(stretchEntryNote(0), null);
});

test("the hover line names the count, singular and plural", () => {
  assert.equal(stretchHoverLine(2), "Stretching turns 2 shapes into paths");
  assert.equal(stretchHoverLine(1), "Stretching turns 1 shape into a path");
});

test("the readout line says how many shapes become paths", () => {
  assert.equal(stretchReadoutLine(2), "2 shapes become paths");
  assert.equal(stretchReadoutLine(1), "1 shape becomes a path");
});

test("the notice states the count in plain digits and that there is no undo", () => {
  assert.equal(
    stretchNotice(2),
    "Stretching turned 2 shapes into paths. No undo yet.",
  );
  assert.equal(
    stretchNotice(1),
    "Stretching turned 1 shape into a path. No undo yet.",
  );
  assert.equal(
    stretchNotice(5000),
    "Stretching turned 5000 shapes into paths. No undo yet.",
  );
});

test("the entry note", () => {
  assert.equal(stretchEntryNote(2), "Turns 2 shapes into paths.");
  assert.equal(stretchEntryNote(1), "Turns 1 shape into a path.");
});

test("the notice stays 6 s", () => {
  assert.equal(CONVERSION_NOTICE_MS, 6000);
});
