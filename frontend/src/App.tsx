import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { Canvas } from "@/components/Canvas";
import { OpenProjectErrorDialog } from "@/components/OpenProjectErrorDialog";
import { StatusBar } from "@/components/StatusBar";
import type { OpenErrorPayload, ProjectStatePayload } from "@/lib/projectState";

const DEFAULT_SIZE_MM = { width: 210, height: 297 };

function App() {
  const [cursorMm, setCursorMm] = useState({ x: 0, y: 0 });
  const [sizeMm, setSizeMm] = useState(DEFAULT_SIZE_MM);
  const [openErrorMessage, setOpenErrorMessage] = useState<string | null>(
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

    const unlistenState = listen<ProjectStatePayload>(
      "project-state",
      (event) => setSizeMm(event.payload.size_mm),
    );
    const unlistenError = listen<OpenErrorPayload>("open-error", (event) =>
      setOpenErrorMessage(event.payload.message),
    );

    return () => {
      unlistenState.then((unlisten) => unlisten());
      unlistenError.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden">
      <Canvas onPointerPositionChange={setCursorMm} />
      <StatusBar cursorMm={cursorMm} sizeMm={sizeMm} />
      <OpenProjectErrorDialog
        message={openErrorMessage}
        onDismiss={() => setOpenErrorMessage(null)}
      />
    </div>
  );
}

export default App;
