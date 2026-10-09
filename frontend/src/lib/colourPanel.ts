// What the colour picker needs of the Style panel.

import type { StyleFieldName, StylePanelApi, TextOutcome } from "@/hooks/useStylePanel";

/** What the colour picker needs of the panel, with the fields as plain names. */
export interface ColourPanel {
  setText: (field: string, text: string) => TextOutcome;
  previewColor: (field: string, rgb: number) => void;
  previewOpacity: (field: string, percent: number) => void;
  previewing: boolean;
  cancels: number;
}

/** The picker's view of the panel for a stroke or fill property. */
export function colourPanelOf(panel: StylePanelApi): ColourPanel {
  return {
    setText: (field, text) => panel.setText(field as StyleFieldName, text),
    previewColor: (field, rgb) => panel.previewColor(field as StyleFieldName, rgb),
    previewOpacity: (field, percent) => panel.previewOpacity(field as StyleFieldName, percent),
    previewing: panel.previewing,
    cancels: panel.cancels,
  };
}
