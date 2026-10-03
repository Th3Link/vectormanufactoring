import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { Canvas } from "@/components/Canvas";
import { ErrorDialog } from "@/components/ErrorDialog";
import { NodeToolbar } from "@/components/NodeToolbar";
import { StatusBar } from "@/components/StatusBar";
import { ToolRail } from "@/components/ToolRail";
import { useEditorSession } from "@/hooks/useEditorSession";
import type {
  OpenErrorPayload,
  ProjectStatePayload,
  SaveErrorPayload,
} from "@/lib/projectState";

const DEFAULT_SIZE_MM = { width: 210, height: 297 };

function App() {
  const [cursorMm, setCursorMm] = useState({ x: 0, y: 0 });
  const [sizeMm, setSizeMm] = useState(DEFAULT_SIZE_MM);
  const editor = useEditorSession(setCursorMm);
  const [openErrorMessage, setOpenErrorMessage] = useState<string | null>(
    null,
  );
  const [saveErrorMessage, setSaveErrorMessage] = useState<string | null>(
    null,
  );

  useEffect(() => {
    // Seed the initial state once on mount rather than relying on an
    // event that may have fired before this listener was attached.
    invoke<ProjectStatePayload>("get_project_state")
      .then((state) => setSizeMm(state.size_mm))
      .catch(() => {
        /* Keep the A4 default if the host is somehow unreachable. */
      });

    // The file-association launch path runs its open attempt in Tauri's
    // `.setup()`, before this effect's listeners below can possibly have
    // attached yet. An open-error from that path has nowhere live to go,
    // so the host buffers it and this is the one-time poll that picks it
    // up — mirroring the `get_project_state` call just above.
    invoke<OpenErrorPayload | null>("take_pending_open_error")
      .then((pending) => {
        if (pending) {
          setOpenErrorMessage(pending.message);
        }
      })
      .catch(() => {
        /* Nothing pending is indistinguishable from an unreachable host. */
      });

    const unlistenState = listen<ProjectStatePayload>(
      "project-state",
      (event) => setSizeMm(event.payload.size_mm),
    );
    const unlistenOpenError = listen<OpenErrorPayload>(
      "open-error",
      (event) => setOpenErrorMessage(event.payload.message),
    );
    const unlistenSaveError = listen<SaveErrorPayload>(
      "save-error",
      (event) => setSaveErrorMessage(event.payload.message),
    );

    return () => {
      unlistenState.then((unlisten) => unlisten());
      unlistenOpenError.then((unlisten) => unlisten());
      unlistenSaveError.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden">
      {editor.tool === "node" ? (
        <NodeToolbar
          state={editor.nodeToolbarState}
          actions={{
            insertSelected: editor.insertSelected,
            deleteSelected: editor.deleteSelected,
            convertSelected: editor.convertSelected,
            makeLine: editor.makeLine,
            makeCurve: editor.makeCurve,
          }}
        />
      ) : null}
      <div className="flex min-h-0 flex-1">
        <ToolRail tool={editor.tool} onSelect={editor.setTool} />
        <Canvas editor={editor} />
      </div>
      <StatusBar cursorMm={cursorMm} sizeMm={sizeMm} />
      <ErrorDialog
        title="Can't open project"
        message={openErrorMessage}
        onDismiss={() => setOpenErrorMessage(null)}
      />
      <ErrorDialog
        title="Can't save project"
        message={saveErrorMessage}
        onDismiss={() => setSaveErrorMessage(null)}
      />
    </div>
  );
}

export default App;
