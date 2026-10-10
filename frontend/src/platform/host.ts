// The one place the frontend talks to its host. Inside the Tauri desktop
// shell `invoke`/`listen` are Tauri's own, unchanged. In a plain browser (the
// public demo build, docs/deploy-demo.md) there is no host: the same two
// functions are served by a tiny in-page implementation, so `App` runs
// the same code on both. Browser Open reads a picked file, Save downloads it.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import {
  listen as tauriListen,
  type EventCallback,
  type UnlistenFn,
} from "@tauri-apps/api/event";

/** True inside the Tauri desktop shell, false in a plain browser. */
export const isTauri = "__TAURI_INTERNALS__" in window;

const listeners = new Map<string, Set<EventCallback<never>>>();

/** Delivers `payload` to every browser-side `listen` handler of `event`. */
export function emitLocal(event: string, payload?: unknown) {
  for (const handler of listeners.get(event) ?? []) {
    (handler as EventCallback<unknown>)({
      event,
      id: 0,
      payload,
    });
  }
}

function download(bytes: number[]) {
  const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
  const link = document.createElement("a");
  link.href = url;
  link.download = "untitled.curvyo";
  link.click();
  URL.revokeObjectURL(url);
}

/** Where the browser build keeps the text of the formats file
 * (`specs/0045-document-formats-library/` criterion 31). Every access is
 * guarded: a failure means the built-in formats only. */
const FORMATS_KEY = "curvyo.document-formats";

function readStored(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Browser Import: a picked `.toml` file's name and text, or `null` when the
 * picker was cancelled. */
function pickTextFile(): Promise<{ name: string; text: string } | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".toml";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      file.text().then((text) => resolve({ name: file.name, text }), reject);
    };
    input.addEventListener("cancel", () => resolve(null));
    input.click();
  });
}

function downloadText(name: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}

const browserCommands: Record<string, (args?: Record<string, unknown>) => unknown> = {
  take_pending_open: () => null,
  confirm_project_opened: () => undefined,
  save_project_bytes: (args) => download(args?.bytes as number[]),
  read_formats_file: () => readStored(FORMATS_KEY),
  write_formats_file: (args) => window.localStorage.setItem(FORMATS_KEY, String(args?.text)),
  set_formats_file_aside: () => {
    const text = readStored(FORMATS_KEY);
    if (text !== null) {
      window.localStorage.setItem(`${FORMATS_KEY}.broken`, text);
    }
    window.localStorage.removeItem(FORMATS_KEY);
  },
  import_formats_file: () => pickTextFile(),
  export_formats_file: (args) => {
    downloadText("curvyo-formats.toml", String(args?.text));
    return true;
  },
};

/** Same contract as Tauri's `invoke`. */
export function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri) {
    return tauriInvoke<T>(command, args);
  }
  const handler = browserCommands[command];
  if (!handler) {
    return Promise.reject(new Error(`${command} is not available in the browser`));
  }
  try {
    return Promise.resolve(handler(args) as T);
  } catch (error) {
    // A guarded storage access that fails is a rejected command, not a throw.
    return Promise.reject(error);
  }
}

/** Same contract as Tauri's `listen`. */
export function listen<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (isTauri) {
    return tauriListen<T>(event, handler);
  }
  const set = listeners.get(event) ?? new Set();
  set.add(handler as EventCallback<never>);
  listeners.set(event, set);
  return Promise.resolve(() => set.delete(handler as EventCallback<never>));
}

/** Browser-only File > Open: asks for a `.curvyo` file and hands its bytes
 * to the same `open-bytes` listener the native menu feeds. */
export function pickProjectFile() {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".curvyo";
  input.onchange = () => {
    const file = input.files?.[0];
    if (!file) {
      return;
    }
    file.arrayBuffer().then(
      (buffer) =>
        emitLocal("open-bytes", {
          path: file.name,
          bytes: Array.from(new Uint8Array(buffer)),
        }),
      () => emitLocal("open-error", { message: "This file couldn't be read." }),
    );
  };
  input.click();
}

/** Why this browser cannot run the editor, or `null` if it can. */
export function unsupportedReason(): string | null {
  if (typeof WebAssembly === "undefined") {
    return "This browser has no WebAssembly support, which Curvyo needs.";
  }
  if (!document.createElement("canvas").getContext("webgl2")) {
    return "This browser or device has no WebGL 2, which Curvyo needs to draw. Try a current Chrome, Edge, Firefox or Safari with hardware acceleration on.";
  }
  return null;
}
