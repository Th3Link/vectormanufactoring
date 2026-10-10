import { useEffect, useState } from "react";

/** How long the text must stay the same before it is spoken, ms: at most one
 * announcement per 500 ms (`specs/0019-multi-object-transform/` UX notes,
 * section 13). */
const SETTLE_MS = 500;

interface SelectionAnnouncerProps {
  /** `curvyo-editor-wasm`'s `selection_announcement()`: "4 objects selected,
   * 46.2 by 18.7 mm", empty for fewer than two objects. */
  text: string;
}

/**
 * A visually hidden polite live region: it speaks the one fact the canvas
 * draws for a multi-selection, how many objects and how big a box. It adds no
 * visible control.
 */
export function SelectionAnnouncer({ text }: SelectionAnnouncerProps) {
  const [spoken, setSpoken] = useState("");
  useEffect(() => {
    const timer = window.setTimeout(() => setSpoken(text), SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, [text]);
  return (
    <div aria-live="polite" role="status" className="sr-only">
      {spoken}
    </div>
  );
}
