// What the colour picker needs of the Style panel, and the adapters that let a
// stop row stand in for a stroke or fill property.

import type {
  StopFieldName,
  StyleFieldName,
  StylePanelApi,
  TextOutcome,
} from "@/hooks/useStylePanel";

/** What the colour picker needs of the panel, with the fields as plain names so
 * a stop row can stand in for a stroke or fill property. */
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

/** The picker's view of the panel for the stop of `rank`: the fields it names
 * are `"color"` and `"opacity"`. */
export function stopColourPanel(panel: StylePanelApi, rank: number): ColourPanel {
  return {
    setText: (field, text) => panel.setStopText(rank, field as StopFieldName, text),
    previewColor: (field, rgb) => panel.previewStop(rank, field as StopFieldName, rgb),
    previewOpacity: (field, percent) => panel.previewStop(rank, field as StopFieldName, percent),
    previewing: panel.previewing,
    cancels: panel.cancels,
  };
}
