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

const browserCommands: Record<string, (args?: Record<string, unknown>) => unknown> = {
  take_pending_open: () => null,
  confirm_project_opened: () => undefined,
  save_project_bytes: (args) => download(args?.bytes as number[]),
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
  return handler
    ? Promise.resolve(handler(args) as T)
    : Promise.reject(new Error(`${command} is not available in the browser`));
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
