// Mirrors the payloads vecmanf-app (src-tauri) sends across the IPC
// boundary: `ProjectStatePayload` / `OpenErrorPayload` in
// vecmanf-app/src/main.rs. Keep these in sync with that file — there is
// no shared schema generator for this slice.

/** The document's page size in millimetres (ADR 0002 §2). */
export interface SizeMm {
  width: number;
  height: number;
}

/** What `get_project_state` returns and what a `project-state` event carries. */
export interface ProjectStatePayload {
  size_mm: SizeMm;
}

/**
 * What an `open-error` event carries (acceptance criterion 7), and what
 * `take_pending_open_error` returns (or `null`/`undefined` if nothing is
 * pending) for the file-association launch race.
 */
export interface OpenErrorPayload {
  message: string;
}

/** What a `save-error` event carries. */
export interface SaveErrorPayload {
  message: string;
}
