/**
 * The Shift, Ctrl and Alt state a keyboard event leaves behind
 * (`specs/advanced-selection/`: a marquee or lasso must follow a modifier key
 * with the pointer at rest).
 *
 * `KeyboardEvent.shiftKey` and its siblings are not reliable on the event of
 * the modifier key itself: some engines (WebKitGTK, the Tauri webview on
 * Linux, reads them from the state *before* the key changed) report the key's
 * own flag as it was, so a Shift keydown says `shiftKey: false` and the
 * following keyup still says `true`. Every other key's flags are right. The
 * type of the event and its `key` say what the modifier key just did, so they
 * decide that one flag; the other two flags are read as reported (they
 * describe keys that did not change).
 */
export interface HeldModifiers {
  shift: boolean;
  /** Ctrl, or Cmd on macOS. */
  ctrl: boolean;
  alt: boolean;
}

export function modifiersAfterKeyEvent(event: KeyboardEvent): HeldModifiers {
  const pressed = event.type === "keydown";
  return {
    shift: event.key === "Shift" ? pressed : event.shiftKey,
    ctrl:
      event.key === "Control"
        ? pressed || event.metaKey
        : event.key === "Meta"
          ? pressed || event.ctrlKey
          : event.ctrlKey || event.metaKey,
    alt: event.key === "Alt" ? pressed : event.altKey,
  };
}
