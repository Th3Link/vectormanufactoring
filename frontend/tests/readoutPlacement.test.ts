// Run with `npm test`. `placeAway` puts the Pen's hint chip on the side of the pointer away from
// the path in progress (`src/lib/readoutPlacement.ts`).

import assert from "node:assert/strict";
import { test } from "node:test";

import { placeAway } from "../src/lib/readoutPlacement.ts";

const chip = { width: 100, height: 40 };
const canvas = { width: 800, height: 600 };
const at = { x: 400, y: 300 };

test("the chip sits on the side the direction points to", () => {
  assert.deepEqual(placeAway(at, chip, canvas, 12, { x: 1, y: 1 }), { left: 412, top: 312 });
  assert.deepEqual(placeAway(at, chip, canvas, 12, { x: -1, y: 1 }), { left: 288, top: 312 });
  assert.deepEqual(placeAway(at, chip, canvas, 12, { x: 1, y: -1 }), { left: 412, top: 248 });
  assert.deepEqual(placeAway(at, chip, canvas, 12, { x: -1, y: -1 }), { left: 288, top: 248 });
});

test("the chip stays inside the canvas", () => {
  const edge = placeAway({ x: 10, y: 10 }, chip, canvas, 12, { x: -1, y: -1 });
  assert.deepEqual(edge, { left: 0, top: 0 });
});
