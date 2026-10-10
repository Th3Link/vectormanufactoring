import { useCallback, useEffect, useRef, useState } from "react";

import { NUDGE_READOUT_MS, NUDGE_RUN_END_MS, selectedAnnouncement } from "@/lib/nudgeText";

/** The move readout of a nudge step, canvas-relative CSS pixels at the selection box centre. */
export interface NudgeReadout {
  text: string;
  x: number;
  y: number;
}

/** What the session says about a nudge, read after each key event. */
interface NudgeSource {
  nudge_readout(): { text: string; anchor_x: number; anchor_y: number; free(): void } | undefined;
  nudge_announcement(): string;
}

export interface NudgeFeedback {
  /** The chip beside the selection box, or `null` 800 ms after the last move. */
  readout: NudgeReadout | null;
  /** The text of the visually hidden live region. */
  announcement: string;
  /** A nudge event was handled: `newRun` is true when it opened a new step. */
  onNudge: (session: NudgeSource, newRun: boolean) => void;
  /** Ctrl+A selected `count` objects. */
  onSelectAll: (count: number) => void;
  /** A key was released: an arrow key ends the step that is running. */
  onKeyUp: (key: string) => void;
}

/**
 * The feedback of a keyboard nudge and of select all (`specs/0044-editing-quick-wins/`
 * criteria 4 and 9): the move readout, which goes 800 ms after the last move, and one hidden
 * announcement per step instead of one per repeat event. A step ends at the arrow key's release,
 * or 600 ms after the last event when the release was missed.
 */
export function useNudgeFeedback(): NudgeFeedback {
  const [readout, setReadout] = useState<NudgeReadout | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const readoutTimer = useRef<number | undefined>(undefined);
  const endTimer = useRef<number | undefined>(undefined);
  const spoken = useRef(0);
  /** The announcement of the running step, until it has been spoken. */
  const pending = useRef("");

  const announce = useCallback((text: string) => {
    if (text === "") {
      return;
    }
    // The same words twice in a row are not re-read unless the text changes.
    spoken.current += 1;
    setAnnouncement(spoken.current % 2 === 0 ? `${text} ` : text);
  }, []);

  const endStep = useCallback(() => {
    window.clearTimeout(endTimer.current);
    announce(pending.current);
    pending.current = "";
  }, [announce]);

  const onNudge = useCallback(
    (session: NudgeSource, newRun: boolean) => {
      if (newRun) {
        endStep();
      }
      pending.current = session.nudge_announcement();
      const raw = session.nudge_readout();
      if (raw) {
        setReadout({ text: raw.text, x: raw.anchor_x, y: raw.anchor_y });
        raw.free();
      }
      window.clearTimeout(readoutTimer.current);
      readoutTimer.current = window.setTimeout(() => setReadout(null), NUDGE_READOUT_MS);
      window.clearTimeout(endTimer.current);
      endTimer.current = window.setTimeout(endStep, NUDGE_RUN_END_MS);
    },
    [endStep],
  );

  const onSelectAll = useCallback(
    (count: number) => announce(selectedAnnouncement(count)),
    [announce],
  );

  const onKeyUp = useCallback(
    (key: string) => {
      if (key.startsWith("Arrow")) {
        endStep();
      }
    },
    [endStep],
  );

  useEffect(
    () => () => {
      window.clearTimeout(readoutTimer.current);
      window.clearTimeout(endTimer.current);
    },
    [],
  );

  return { readout, announcement, onNudge, onSelectAll, onKeyUp };
}
