// Run with `npm test`. The position of a value-field drag on its scale:
// criterion 44 (a mixed field is absolute under the pointer) and criterion 38
// (any other is relative to the value at the press).

import assert from "node:assert/strict";
import { test } from "node:test";

import { dragPosition, type DragFrame } from "../src/lib/dragPosition.ts";

const FIELD = { left: 100, width: 244 };

function frame(over: Partial<DragFrame>): DragFrame {
  return { mixed: false, baseP: 0, baseX: 0, factor: 1, ...FIELD, ...over };
}

test("a mixed drag is the pointer's position in the field, whatever the press", () => {
  // The spec example: pressed at 560, released at 700 in a field starting at 533.
  const field = frame({ mixed: true, baseX: 560, left: 533 });
  assert.ok(Math.abs(dragPosition(field, 700) - (700 - 533) / 244) < 1e-12);
  assert.ok(dragPosition(field, 700) > dragPosition(field, 560));
});

test("a mixed drag to the left of the field is 0 and to the right is 1", () => {
  const mixed = frame({ mixed: true });
  assert.equal(dragPosition(mixed, 40), 0);
  assert.equal(dragPosition(mixed, 900), 1);
});

test("a relative drag moves from the value at the press, scaled by the modifier", () => {
  const relative = frame({ baseP: 0.5, baseX: 200, factor: 1 });
  assert.equal(dragPosition(relative, 200 + 24.4), 0.6);
  assert.ok(Math.abs(dragPosition({ ...relative, factor: 10 }, 200 + 12.2) - 1) < 1e-12);
  assert.equal(dragPosition({ ...relative, factor: 0.1 }, 200 + 244), 0.6);
});

test("a relative drag clamps at the ends", () => {
  const relative = frame({ baseP: 0.9, baseX: 200 });
  assert.equal(dragPosition(relative, 5000), 1);
  assert.equal(dragPosition(relative, -5000), 0);
});
