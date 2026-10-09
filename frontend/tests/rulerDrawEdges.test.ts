// Run with `npm test`. Independent tester tests for the pure drawing of a
// ruler strip (`src/lib/rulerDraw.ts`), `specs/0015-document-size-and-rulers`
// criteria 2, 5 to 8 and the design-system rows "Ruler", "Ruler ticks",
// "Ruler pointer marker". Written from the specification, against a recording
// stub of the Canvas2D context.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  type RulerColours,
  type RulerContext,
  type RulerData,
  type RulerStrip,
  LABEL_OFFSET,
  MINOR_TICK_DEPTH,
  RULER_THICKNESS_PX,
  drawRuler,
} from "../src/lib/rulerDraw.ts";

const C: RulerColours = {
  background: "bg",
  tick: "tick",
  tickMinor: "minor",
  label: "label",
  edge: "edge",
  pointer: "pointer",
};

type Op =
  | { kind: "rect"; x: number; y: number; w: number; h: number; colour: string }
  | { kind: "text"; text: string; x: number; y: number; rotated: boolean; align: string };

function record(): { ctx: RulerContext; ops: Op[] } {
  const ops: Op[] = [];
  let rotated = false;
  const ctx: RulerContext = {
    fillStyle: "",
    font: "",
    textBaseline: "alphabetic",
    textAlign: "start",
    fillRect(x, y, w, h) {
      ops.push({ kind: "rect", x, y, w, h, colour: String(ctx.fillStyle) });
    },
    fillText(text, x, y) {
      ops.push({ kind: "text", text, x, y, rotated, align: ctx.textAlign });
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
  return { ctx, ops };
}

function strip(over: Partial<RulerStrip> = {}): RulerStrip {
  return {
    horizontal: true,
    length: 500,
    thickness: RULER_THICKNESS_PX,
    devicePixelRatio: 1,
    pointer: null,
    ...over,
  };
}

const DATA: RulerData = {
  majors: [-20.3, 55.5, 130.7, 205.9],
  minors: [-4.3, 11.2, 26.6, 42.1, 70.9, 86.3],
  origin: 55.5,
  labelTicks: [-20.3, 55.5, 130.7],
  labelTexts: ["−2", "0", "2"],
};

const rects = (ops: Op[], colour: string) =>
  ops.filter((o): o is Extract<Op, { kind: "rect" }> => o.kind === "rect" && o.colour === colour);

test("every tick and the marker lie on whole device pixels at any pixel ratio", () => {
  for (const dpr of [1, 1.25, 1.5, 2, 2.5, 3]) {
    for (const horizontal of [true, false]) {
      const { ctx, ops } = record();
      drawRuler(ctx, DATA, strip({ horizontal, devicePixelRatio: dpr, pointer: 77.77 }), C);
      for (const colour of ["tick", "minor", "pointer"]) {
        for (const r of rects(ops, colour)) {
          for (const v of [r.x, r.y, r.w, r.h]) {
            assert.equal(v, Math.round(v), `${colour} @${dpr} ${horizontal}: ${JSON.stringify(r)}`);
          }
        }
      }
    }
  }
});

test("the origin tick is two device pixels wide, every other major one", () => {
  for (const dpr of [1, 2, 3]) {
    const { ctx, ops } = record();
    drawRuler(ctx, DATA, strip({ devicePixelRatio: dpr }), C);
    const majors = rects(ops, "tick");
    assert.equal(majors.length, DATA.majors.length);
    const widths = majors.map((r) => r.w);
    assert.equal(widths.filter((w) => w === 2 * dpr).length, 1, `@${dpr}: ${widths}`);
    assert.equal(widths.filter((w) => w === dpr).length, 3);
  }
});

test("without an origin on the strip no tick is widened", () => {
  const { ctx, ops } = record();
  drawRuler(ctx, { ...DATA, origin: undefined }, strip(), C);
  assert.ok(rects(ops, "tick").every((r) => r.w === 1));
});

test("major ticks span the full thickness, minors 8 px, both grow from the canvas-facing edge", () => {
  for (const dpr of [1, 2]) {
    const t = RULER_THICKNESS_PX * dpr;
    const h = record();
    drawRuler(h.ctx, DATA, strip({ devicePixelRatio: dpr }), C);
    for (const r of rects(h.ops, "tick")) {
      assert.equal(r.h, t);
      assert.equal(r.y + r.h, t);
    }
    for (const r of rects(h.ops, "minor")) {
      assert.equal(r.h, MINOR_TICK_DEPTH * dpr);
      assert.equal(r.y + r.h, t, "minor ticks hang from the bottom edge of the top ruler");
    }
    const v = record();
    drawRuler(v.ctx, DATA, strip({ horizontal: false, devicePixelRatio: dpr, length: 500 }), C);
    for (const r of rects(v.ops, "tick")) {
      assert.equal(r.w, t);
      assert.equal(r.h === 1 * dpr || r.h === 2 * dpr, true);
      assert.equal(r.x, 0, "major tick of the left ruler spans its full thickness");
    }
    for (const r of rects(v.ops, "minor")) {
      assert.equal(r.w, MINOR_TICK_DEPTH * dpr);
      assert.equal(r.x + r.w, t, "minor ticks grow from the right edge of the left ruler");
    }
  }
});

test("the marker is one device pixel across the full thickness and only when the pointer is inside", () => {
  for (const dpr of [1, 1.5, 2]) {
    const none = record();
    drawRuler(none.ctx, DATA, strip({ devicePixelRatio: dpr, pointer: null }), C);
    assert.equal(rects(none.ops, "pointer").length, 0);

    const some = record();
    drawRuler(some.ctx, DATA, strip({ devicePixelRatio: dpr, pointer: 100.2 }), C);
    const marks = rects(some.ops, "pointer");
    assert.equal(marks.length, 1);
    assert.equal(marks[0].w, Math.max(1, Math.round(dpr)));
    assert.equal(marks[0].h, RULER_THICKNESS_PX * dpr);
  }
});

test("pointer at position 0 still draws a marker (0 is not 'absent')", () => {
  const { ctx, ops } = record();
  drawRuler(ctx, DATA, strip({ pointer: 0 }), C);
  assert.equal(rects(ops, "pointer").length, 1);
});

test("paint order: ground, minors, majors, marker, edge line, then labels", () => {
  const { ctx, ops } = record();
  drawRuler(ctx, DATA, strip({ pointer: 100 }), C);
  const firstIndex = (colour: string) =>
    ops.findIndex((o) => o.kind === "rect" && o.colour === colour);
  assert.equal(firstIndex("bg"), 0);
  assert.ok(firstIndex("minor") < firstIndex("tick"));
  assert.ok(firstIndex("tick") < firstIndex("pointer"), "marker above the ticks");
  const firstText = ops.findIndex((o) => o.kind === "text");
  assert.ok(firstText > firstIndex("pointer"), "labels above the marker");
  assert.ok(firstText > firstIndex("edge"));
});

test("horizontal labels start 4 px right of the tick and carry the text unchanged", () => {
  for (const dpr of [1, 2]) {
    const { ctx, ops } = record();
    drawRuler(ctx, DATA, strip({ devicePixelRatio: dpr }), C);
    const texts = ops.filter((o): o is Extract<Op, { kind: "text" }> => o.kind === "text");
    assert.deepEqual(
      texts.map((t) => t.text),
      DATA.labelTexts,
    );
    texts.forEach((t, i) => {
      assert.equal(t.x, Math.round((DATA.labelTicks[i] + LABEL_OFFSET) * dpr));
      assert.equal(t.rotated, false);
    });
  }
  assert.equal(LABEL_OFFSET, 4);
});

test("vertical labels are rotated and keep the real minus sign", () => {
  const { ctx, ops } = record();
  drawRuler(ctx, DATA, strip({ horizontal: false }), C);
  const texts = ops.filter((o): o is Extract<Op, { kind: "text" }> => o.kind === "text");
  assert.equal(texts.length, DATA.labelTexts.length);
  assert.ok(texts.every((t) => t.rotated));
  assert.equal(texts[0].text.charCodeAt(0), 0x2212);
  assert.ok(texts.every((t) => !t.text.includes("-")));
});

test("an empty layout paints the ground and the edge only, and does not throw", () => {
  const empty: RulerData = {
    majors: [],
    minors: [],
    origin: undefined,
    labelTicks: [],
    labelTexts: [],
  };
  for (const horizontal of [true, false]) {
    const { ctx, ops } = record();
    drawRuler(ctx, empty, strip({ horizontal }), C);
    assert.deepEqual(
      ops.map((o) => (o.kind === "rect" ? o.colour : "text")),
      ["bg", "edge"],
    );
  }
});

test("a very large layout draws quickly and without throwing", () => {
  const n = 20000;
  const majors = Array.from({ length: n }, (_, i) => i * 3.7 - 1000);
  const { ctx, ops } = record();
  const t0 = performance.now();
  drawRuler(
    ctx,
    { majors, minors: majors, origin: undefined, labelTicks: [], labelTexts: [] },
    strip({ length: 80000 }),
    C,
  );
  assert.ok(performance.now() - t0 < 2000);
  assert.ok(ops.length >= 2 * n);
});

test("ticks off the strip (negative or past the end) are drawn without NaN geometry", () => {
  const { ctx, ops } = record();
  drawRuler(
    ctx,
    { ...DATA, majors: [-39.9, -0.4, 500.4, 539.9], minors: [], origin: undefined, labelTicks: [], labelTexts: [] },
    strip(),
    C,
  );
  for (const o of ops) {
    if (o.kind === "rect") {
      for (const v of [o.x, o.y, o.w, o.h]) {
        assert.ok(Number.isFinite(v));
      }
    }
  }
});

test(
  "the marker's centre is within half a pixel of the pointer position",
  () => {
    for (let k = 0; k < 100; k += 1) {
      const pointer = 100 + k / 100;
      const { ctx, ops } = record();
      drawRuler(ctx, DATA, strip({ pointer }), C);
      const mark = rects(ops, "pointer")[0];
      const centre = mark.x + mark.w / 2;
      assert.ok(Math.abs(centre - pointer) <= 0.5 + 1e-9, `pointer ${pointer}: centre ${centre}`);
    }
  },
);

test(
  "a major tick's centre is within half a pixel of its projected position (criterion 2)",
  () => {
    for (let k = 0; k < 100; k += 1) {
      const px = 100 + k / 100;
      const { ctx, ops } = record();
      drawRuler(ctx, { majors: [px], minors: [], origin: undefined, labelTicks: [], labelTexts: [] }, strip(), C);
      const tick = rects(ops, "tick")[0];
      assert.ok(Math.abs(tick.x + tick.w / 2 - px) <= 0.5 + 1e-9, `px ${px}: centre ${tick.x + tick.w / 2}`);
    }
  },
);
