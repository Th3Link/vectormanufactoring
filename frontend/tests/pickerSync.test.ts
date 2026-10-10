// Run with `npm test`. `pickerStep` is the rule that makes the inline colour picker read the
// colour again (`src/lib/pickerSync.ts`); the Background block depends on it because it mounts
// before the wasm session exists (`specs/0040-document-background`, UX review B1).

import assert from "node:assert/strict";
import { test } from "node:test";

import { pickerStep } from "../src/lib/pickerSync.ts";

const DEFAULT = 0xe8e8eb;
const coldConverter = () => [-1, 0, 0];
const readyConverter = () => [240, 0.01, 0.92];

test("a picker mounted before the session exists derives again once the session is there", () => {
  // Cold: nothing read yet, the placeholder converter reports the default colour.
  const cold = { rgb: null, source: null };
  assert.equal(pickerStep(cold, DEFAULT, null, coldConverter, false), "derive");
  // After the first read the colour alone changes nothing...
  const afterColdRead = { rgb: DEFAULT, source: coldConverter };
  assert.equal(pickerStep(afterColdRead, DEFAULT, null, coldConverter, false), "keep");
  // ...but the real converter, with the very same colour, makes it derive from the session.
  assert.equal(pickerStep(afterColdRead, DEFAULT, null, readyConverter, false), "derive");
  const afterReadyRead = { rgb: DEFAULT, source: readyConverter };
  assert.equal(pickerStep(afterReadyRead, DEFAULT, null, readyConverter, false), "keep");
});

test("a colour changed from outside is derived, one the picker sent itself is only noted", () => {
  const seen = { rgb: 0x112233, source: readyConverter };
  assert.equal(pickerStep(seen, 0x445566, null, readyConverter, false), "derive");
  assert.equal(pickerStep(seen, 0x445566, 0x445566, readyConverter, false), "note");
  assert.equal(pickerStep(seen, 0x112233, 0x445566, readyConverter, false), "keep");
});

test("a mixed colour reads nothing", () => {
  const seen = { rgb: null, source: null };
  assert.equal(pickerStep(seen, 0, null, readyConverter, true), "keep");
});
