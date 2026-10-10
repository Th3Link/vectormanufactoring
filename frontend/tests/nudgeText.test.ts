import assert from "node:assert/strict";
import { test } from "node:test";

import {
  NUDGE_READOUT_MS,
  NUDGE_RUN_END_MS,
  SELECT_KEYS_TOOLTIP,
  selectedAnnouncement,
} from "../src/lib/nudgeText.ts";

test("select all is announced with the count, singular for one", () => {
  assert.equal(selectedAnnouncement(5), "Selected 5 objects.");
  assert.equal(selectedAnnouncement(1), "Selected 1 object.");
  assert.equal(selectedAnnouncement(5000), "Selected 5000 objects.");
});

test("an empty selection announces nothing", () => {
  assert.equal(selectedAnnouncement(0), "");
});

test("the tooltip line names both keys and both distances", () => {
  assert.equal(SELECT_KEYS_TOOLTIP, "Arrows nudge 1 mm, Shift 10 mm. Ctrl+A selects all.");
});

test("the readout outlives the run window", () => {
  assert.ok(NUDGE_READOUT_MS > NUDGE_RUN_END_MS);
});
