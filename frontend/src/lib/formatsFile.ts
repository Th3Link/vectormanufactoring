// The text of the maker's formats file, `document-formats.toml`
// (`specs/0045-document-formats-library/`, `adrs.md` decision 5). The host
// (Tauri commands, or `localStorage` in the browser build) only moves text; the
// library in `curvyo-document-core` parses and writes it, inside each
// `WasmSession`. This module reads the text once before the first session
// exists, hands it to every new session (New and Open make a fresh one), and
// keeps it current after each edit.
import type { WasmSession } from "@/lib/editorSession";
import { invoke } from "@/platform/host";

let text: string | null = null;
let readError: string | null = null;
let loading: Promise<void> | null = null;

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Reads the file once. A file that cannot be read at all (not text, no
 * permission) is remembered as broken; a missing file is the built-in
 * defaults. Resolves in every case. */
export function formatsFileReady(): Promise<void> {
  loading ??= invoke<string | null>("read_formats_file").then(
    (read) => {
      text = read;
    },
    (error: unknown) => {
      readError = messageOf(error);
    },
  );
  return loading;
}

/** Gives a new session the formats: the file's text over the built-in list, or
 * the reason the file could not be read. The session keeps the reason
 * (`format_list().broken`). */
export function applyFormats(session: WasmSession): void {
  if (readError !== null) {
    session.mark_formats_broken(readError);
  } else if (text !== null) {
    session.load_formats(text);
  }
}

/** Keeps `newText` as the file's text and writes it. Returns why the write
 * failed, or `null`. The in-memory library stays as it is either way. */
export async function storeFormats(newText: string): Promise<string | null> {
  text = newText;
  try {
    await invoke("write_formats_file", { text: newText });
    return null;
  } catch (error) {
    return messageOf(error);
  }
}

/** Renames the unreadable file aside. Returns why it failed, or `null`. */
export async function setFormatsAside(): Promise<string | null> {
  try {
    await invoke("set_formats_file_aside");
  } catch (error) {
    return messageOf(error);
  }
  text = null;
  readError = null;
  return null;
}

/** A file the maker picked for Import. */
export interface ImportedFormats {
  name: string;
  text: string;
}

/** The OS file picker for Import; `null` when cancelled. */
export function pickFormatsFile(): Promise<ImportedFormats | null> {
  return invoke<ImportedFormats | null>("import_formats_file");
}

/** The OS Save dialog (a download in the browser) for Export; `false` when
 * cancelled. */
export function saveFormatsFile(exportText: string): Promise<boolean> {
  return invoke<boolean>("export_formats_file", { text: exportText });
}
