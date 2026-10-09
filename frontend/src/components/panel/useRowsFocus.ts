import { useEffect, useRef } from "react";
import type { RefObject } from "react";

/**
 * Where focus goes when the rows under a Paint switch leave the tree
 * (`specs/0017-style-panel-rework` criterion 1): a control that had keyboard
 * focus is removed without a blur, so the section remembers whether focus was
 * inside its rows and, when they go, moves it to the section's Paint group.
 * Spread the returned props on the element that wraps the rows.
 */
export function useRowsFocus(rowsShown: boolean, section: RefObject<HTMLElement | null>) {
  const hadFocus = useRef(false);
  useEffect(() => {
    if (!rowsShown && hadFocus.current) {
      hadFocus.current = false;
      const radios = section.current?.querySelectorAll<HTMLElement>('[role="radio"]');
      const target =
        section.current?.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]') ??
        radios?.[0];
      target?.focus({ focusVisible: true } as FocusOptions);
    }
  }, [rowsShown, section]);
  return {
    onFocus: () => {
      hadFocus.current = true;
    },
    onBlur: () => {
      hadFocus.current = false;
    },
  };
}
