// Run with `npm test` (Node's own test runner and type stripping: no
// dependency). The pure part of the modifier tracking of
// `src/lib/keyModifiers.ts`, driven by event sequences as WebKitGTK sends
// them, flags stale on the modifier key's own events included.

import assert from "node:assert/strict";
import { test } from "node:test";

import { ModifierTracker, type KeyEventLike } from "../src/lib/keyModifiers.ts";

const NONE = { shift: false, ctrl: false, alt: false };

function key(
  type: "keydown" | "keyup",
  keyName: string,
  code: string,
  flags: Partial<Pick<KeyEventLike, "shiftKey" | "ctrlKey" | "metaKey" | "altKey">> = {},
): KeyEventLike {
  return {
    type,
    key: keyName,
    code,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    ...flags,
  };
}

test("a Shift keydown whose own flag is stale (false) still holds Shift", () => {
  const t = new ModifierTracker();
  assert.deepEqual(t.keyEvent(key("keydown", "Shift", "ShiftLeft", { shiftKey: false })), {
    ...NONE,
    shift: true,
  });
});

test("a Shift keyup whose own flag is stale (true) releases Shift", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Shift", "ShiftLeft"));
  assert.deepEqual(t.keyEvent(key("keyup", "Shift", "ShiftLeft", { shiftKey: true })), NONE);
});

test("both Shift keys: releasing one while the other is down keeps Shift held", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Shift", "ShiftLeft"));
  // The right key goes down with Shift already held (flag true).
  t.keyEvent(key("keydown", "Shift", "ShiftRight", { shiftKey: true }));
  assert.equal(t.keyEvent(key("keyup", "Shift", "ShiftLeft", { shiftKey: true })).shift, true);
  assert.equal(t.keyEvent(key("keyup", "Shift", "ShiftRight", { shiftKey: true })).shift, false);
});

test("both sides also hold for Ctrl and Alt, and Meta counts as Ctrl", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Control", "ControlLeft"));
  t.keyEvent(key("keydown", "Control", "ControlRight", { ctrlKey: true }));
  assert.equal(t.keyEvent(key("keyup", "Control", "ControlLeft", { ctrlKey: true })).ctrl, true);
  assert.equal(t.keyEvent(key("keyup", "Control", "ControlRight", { ctrlKey: true })).ctrl, false);
  t.keyEvent(key("keydown", "Meta", "MetaLeft"));
  assert.equal(t.held.ctrl, true);
  t.keyEvent(key("keyup", "Meta", "MetaLeft", { metaKey: true }));
  assert.equal(t.held.ctrl, false);
  t.keyEvent(key("keydown", "Alt", "AltLeft"));
  t.keyEvent(key("keydown", "Alt", "AltRight", { altKey: true }));
  assert.equal(t.keyEvent(key("keyup", "Alt", "AltRight", { altKey: true })).alt, true);
  assert.equal(t.keyEvent(key("keyup", "Alt", "AltLeft", { altKey: true })).alt, false);
});

test("the modifiers are independent, and a held one survives another's events", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Shift", "ShiftLeft"));
  // Ctrl goes down while Shift is held: the Ctrl keydown reports Shift true.
  assert.deepEqual(t.keyEvent(key("keydown", "Control", "ControlLeft", { shiftKey: true })), {
    shift: true,
    ctrl: true,
    alt: false,
  });
  // Ctrl released with a stale own flag: Shift stays.
  assert.deepEqual(
    t.keyEvent(key("keyup", "Control", "ControlLeft", { shiftKey: true, ctrlKey: true })),
    { shift: true, ctrl: false, alt: false },
  );
});

test("a repeated keydown (key held) changes nothing", () => {
  const t = new ModifierTracker();
  for (let i = 0; i < 5; i += 1) {
    t.keyEvent(key("keydown", "Shift", "ShiftLeft", { shiftKey: i > 0 }));
  }
  assert.equal(t.held.shift, true);
  assert.equal(t.keyEvent(key("keyup", "Shift", "ShiftLeft", { shiftKey: true })).shift, false);
});

test("the flags of an ordinary key correct a lost key-up", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Shift", "ShiftLeft"));
  // The Shift keyup never arrives (an input method took it); the next ordinary
  // key reports Shift false.
  assert.equal(t.keyEvent(key("keydown", "a", "KeyA")).shift, false);
});

test("an ordinary key reporting a held modifier the tracker never saw remembers it", () => {
  const t = new ModifierTracker();
  assert.equal(t.keyEvent(key("keydown", "a", "KeyA", { shiftKey: true })).shift, true);
  // The keyup of Shift ends it though its keydown was missed.
  assert.equal(t.keyEvent(key("keyup", "Shift", "ShiftLeft", { shiftKey: true })).shift, false);
});

test("pointer flags correct a key-up that was lost", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Shift", "ShiftLeft"));
  assert.deepEqual(t.pointerFlags({ shift: false, ctrl: false, alt: false }), NONE);
  assert.deepEqual(t.pointerFlags({ shift: true, ctrl: false, alt: false }), {
    ...NONE,
    shift: true,
  });
});

test("a window blur or a hidden page forgets every held key", () => {
  const t = new ModifierTracker();
  t.keyEvent(key("keydown", "Shift", "ShiftLeft"));
  t.keyEvent(key("keydown", "Alt", "AltLeft", { shiftKey: true }));
  assert.deepEqual(t.reset(), NONE);
  // The key-ups that arrive later, or never, find nothing held.
  assert.deepEqual(t.keyEvent(key("keyup", "Shift", "ShiftLeft", { shiftKey: true })), NONE);
});

test("a synthetic event without a code is told apart by key and location", () => {
  const t = new ModifierTracker();
  const down = { ...key("keydown", "Shift", ""), location: 1 };
  const up = { ...key("keyup", "Shift", "", { shiftKey: true }), location: 1 };
  assert.equal(t.keyEvent(down).shift, true);
  assert.equal(t.keyEvent(up).shift, false);
});
