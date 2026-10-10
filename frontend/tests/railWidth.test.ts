// Run with `npm test`. The rail's width is written in three places: `--rail-right` in
// `src/index.css`, the tooltip offsets in `src/components/railCard.ts`, and the default view inset
// in `curvyo-ui-core/src/viewport.rs`. This test keeps them in step (`docs/technical-debt.md`).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  COLUMN_A_TOOLTIP_OFFSET_PX,
  COLUMN_B_TOOLTIP_OFFSET_PX,
} from "../src/components/railCard.ts";

const read = (path: string) => readFileSync(new URL(path, import.meta.url), "utf8");

/** Two 48px columns 8px apart, inset 12px from the viewport edge. */
const RAIL_INSET_PX = 12;
const COLUMN_WIDTH_PX = 48;
const COLUMN_GAP_PX = 8;
/** The tooltip opens this far right of the rail's right edge. */
const TOOLTIP_GAP_PX = 6;
/** The buttons sit 4px inside their card. */
const CARD_PADDING_PX = 4;

test("--rail-right is the right edge of the second column", () => {
  const css = read("../src/index.css");
  const railRight = Number(/--rail-right:\s*(\d+)px/.exec(css)?.[1]);
  assert.equal(railRight, RAIL_INSET_PX + 2 * COLUMN_WIDTH_PX + COLUMN_GAP_PX);
});

test("the tooltip offsets reach 6px past --rail-right from each column's buttons", () => {
  const css = read("../src/index.css");
  const railRight = Number(/--rail-right:\s*(\d+)px/.exec(css)?.[1]);
  const columnAButtonRight = RAIL_INSET_PX + COLUMN_WIDTH_PX - CARD_PADDING_PX;
  const columnBButtonRight = railRight - CARD_PADDING_PX;
  assert.equal(columnAButtonRight + COLUMN_A_TOOLTIP_OFFSET_PX, railRight + TOOLTIP_GAP_PX);
  assert.equal(columnBButtonRight + COLUMN_B_TOOLTIP_OFFSET_PX, railRight + TOOLTIP_GAP_PX);
});

test("the default view puts the document 12px right of --rail-right (Rust)", () => {
  const css = read("../src/index.css");
  const railRight = Number(/--rail-right:\s*(\d+)px/.exec(css)?.[1]);
  const rust = read("../../curvyo-ui-core/src/viewport.rs");
  const inset = Number(/DOCUMENT_INSET_PX: f64 = (\d+(?:\.\d+)?);/.exec(rust)?.[1]);
  assert.equal(inset, railRight + 12);
});
