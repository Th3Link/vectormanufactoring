// Run with `npm test`. The pure drawing of a ruler strip
// (`src/lib/rulerDraw.ts`) against a recording stub of the Canvas2D context.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  type RulerColours,
  type RulerContext,
  type RulerData,
  drawRuler,
} from "../src/lib/rulerDraw.ts";

const COLOURS: RulerColours = {
  background: "bg",
  tick: "tick",
  tickMinor: "minor",
  label: "label",
  edge: "edge",
  pointer: "pointer",
};

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
  colour: string;
}
interface Text {
  text: string;
  x: number;
  y: number;
  rotated: boolean;
}

function recorder() {
  const rects: Rect[] = [];
  const texts: Text[] = [];
  let rotated = false;
  const ctx: RulerContext = {
    fillStyle: "",
    font: "",
    textBaseline: "alphabetic",
    textAlign: "start",
    fillRect(x, y, w, h) {
      rects.push({ x, y, w, h, colour: String(ctx.fillStyle) });
    },
    fillText(text, x, y) {
      texts.push({ text, x, y, rotated });
    },
    save() {},
    restore() {
      rotated = false;
    },
    translate() {},
    rotate() {
      rotated = true;
    },
  };
  return { ctx, rects, texts };
}

const DATA: RulerData = {
  majors: [-5, 80, 100, 180],
  minors: [20, 40, 60],
  origin: 100,
  labelTicks: [80, 180],
  labelTexts: ["20", "40"],
};

function strip(over: Partial<Parameters<typeof drawRuler>[2]> = {}) {
  return {
    horizontal: true,
    length: 200,
    thickness: 24,
    devicePixelRatio: 1,
    pointer: null,
    ...over,
  };
}

test("major ticks span the full depth, minor ticks 8 px, from the canvas-facing edge", () => {
  const { ctx, rects } = recorder();
  drawRuler(ctx, DATA, strip(), COLOURS);
  const major = rects.find((r) => r.colour === "tick" && r.x === 80);
  assert.deepEqual([major?.w, major?.y, major?.h], [1, 0, 24]);
  const minor = rects.find((r) => r.colour === "minor" && r.x === 40);
  assert.deepEqual([minor?.w, minor?.y, minor?.h], [1, 16, 8]);
});

test("the tick at the origin is 2 px wide", () => {
  const { ctx, rects } = recorder();
  drawRuler(ctx, DATA, strip(), COLOURS);
  const origin = rects.find((r) => r.colour === "tick" && r.x === 99);
  assert.equal(origin?.w, 2);
  const other = rects.find((r) => r.colour === "tick" && r.x === 180);
  assert.equal(other?.w, 1);
});

test("the pointer marker is one device pixel across the full thickness, below the labels", () => {
  const { ctx, rects, texts } = recorder();
  drawRuler(ctx, DATA, strip({ pointer: 33 }), COLOURS);
  const marker = rects.find((r) => r.colour === "pointer");
  assert.deepEqual([marker?.x, marker?.w, marker?.y, marker?.h], [33, 1, 0, 24]);
  assert.equal(texts.length, 2);
  const withoutPointer = recorder();
  drawRuler(withoutPointer.ctx, DATA, strip(), COLOURS);
  assert.equal(withoutPointer.rects.some((r) => r.colour === "pointer"), false);
});

test("labels start 4 px after their tick, 3 px from the top", () => {
  const { ctx, texts } = recorder();
  drawRuler(ctx, DATA, strip(), COLOURS);
  assert.deepEqual(texts[0], { text: "20", x: 84, y: 3, rotated: false });
});

test("the vertical ruler draws its ticks from the right edge and rotates its labels", () => {
  const { ctx, rects, texts } = recorder();
  drawRuler(ctx, DATA, strip({ horizontal: false, length: 200 }), COLOURS);
  const major = rects.find((r) => r.colour === "tick" && r.y === 80);
  assert.deepEqual([major?.x, major?.w, major?.h], [0, 24, 1]);
  const minor = rects.find((r) => r.colour === "minor" && r.y === 40);
  assert.deepEqual([minor?.x, minor?.w], [16, 8]);
  assert.equal(texts.every((t) => t.rotated), true);
});

test("on a 2x display every position is a whole device pixel", () => {
  const { ctx, rects } = recorder();
  drawRuler(
    ctx,
    { ...DATA, majors: [10.3, 55.6], minors: [20.2], origin: undefined },
    strip({ devicePixelRatio: 2 }),
    COLOURS,
  );
  for (const r of rects) {
    assert.equal(Number.isInteger(r.x) && Number.isInteger(r.y), true, JSON.stringify(r));
  }
  const major = rects.find((r) => r.colour === "tick" && r.x === 21 - 1);
  assert.equal(major?.w, 2, "a 1 px tick is 2 device pixels");
});
