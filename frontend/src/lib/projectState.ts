// Mirrors the payloads curvyo-app (src-tauri) sends across the IPC
// boundary: the various `...Payload` structs in curvyo-app/src/main.rs.
// Keep these in sync with that file — there is no shared schema generator
// for this slice.
//
// `path-node-editing`'s PR review ("the host does byte I/O only") moved
// the actual document to the frontend's own `WasmSession`
// (`@/lib/editorSession`); the host now only ever hands this side raw
// bytes to parse (`WasmSession.open`) or already-packed bytes to write
// (`WasmSession.pack`, then the `save_project_bytes` command) — it never
// builds or reads a document itself.

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
 * A host-level failure to even read a file's bytes (missing, permissions,
 * not a file) — what an `open-error` event carries. Distinct from a
 * `.curvyo` whose *content* is invalid: that refusal is the frontend's own,
 * thrown by `WasmSession.open` itself once it has the bytes.
 */
export interface OpenErrorPayload {
  message: string;
}

/** A file the host could read, for `WasmSession.open` to try parsing —
 * what an `open-bytes` event carries. */
export interface OpenBytesPayload {
  path: string;
  bytes: number[];
}

/**
 * What `take_pending_open` returns (or `null`/`undefined` if nothing is
 * pending) for the file-association launch race: either of the two
 * payloads above, tagged by `kind` so the one listener that handles both
 * live events can handle this too.
 */
export type PendingOpenPayload =
  | ({ kind: "bytes" } & OpenBytesPayload)
  | ({ kind: "io_error" } & OpenErrorPayload);

/** What a `save-error` event carries. */
export interface SaveErrorPayload {
  message: string;
}

/**
 * What a `request-pack` event carries: the host is asking the frontend to
 * pack the live session's document and hand the bytes to
 * `save_project_bytes` — `save_as` forces the native Save As… dialog even
 * when a path is already known (acceptance criterion 3).
 */
export interface RequestPackPayload {
  save_as: boolean;
  app_version: string;
}
