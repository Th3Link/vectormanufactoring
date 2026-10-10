import { useEffect, useState } from "react";

import { Canvas } from "@/components/Canvas";
import { ErrorDialog } from "@/components/ErrorDialog";
import { NodeToolbar } from "@/components/NodeToolbar";
import { PropertiesPanel } from "@/components/panel/PropertiesPanel";
import { Rulers } from "@/components/Rulers";
import { SelectToolbar } from "@/components/SelectToolbar";
import { ShapeToolbar } from "@/components/ShapeToolbar";
import { StatusBar } from "@/components/StatusBar";
import { ToolRail } from "@/components/ToolRail";
import type { EditorSession } from "@/hooks/useEditorSession";
import { useRailCommands } from "@/hooks/useRailCommands";
import { useEditorSession } from "@/hooks/useEditorSession";
import { useClosePathState } from "@/hooks/usePenCue";
import { useDocumentPanel } from "@/hooks/useDocumentPanel";
import type {
  OpenBytesPayload,
  OpenErrorPayload,
  PendingOpenPayload,
  RequestPackPayload,
  SaveErrorPayload,
} from "@/lib/projectState";
import { RULER_THICKNESS_PX } from "@/lib/rulerDraw";
import { BrowserBar } from "@/platform/BrowserBar";
import { invoke, isTauri, listen } from "@/platform/host";

/** Asks `editor` to parse `bytes` as the new live session and, only on
 * success, tells the host to start treating `path` as the open
 * project's own path (`confirm_project_opened` — acceptance criterion 7:
 * a refusal must never touch an already-open project, and this host-side
 * update only ever runs after the parse itself has already succeeded). */
async function openBytes(
  editor: EditorSession,
  path: string,
  bytes: number[],
  setOpenErrorMessage: (message: string) => void,
) {
  try {
    await editor.openProject(new Uint8Array(bytes));
  } catch (error) {
    setOpenErrorMessage(String(error));
    return;
  }
  await invoke("confirm_project_opened", { path }).catch(() => {
    /* The window title/path tracking falls behind, but the document
     * itself opened fine — not worth a user-facing error for. */
  });
}

function App() {
  const [cursorMm, setCursorMm] = useState({ x: 0, y: 0 });
  const editor = useEditorSession(setCursorMm);
  const railCommands = useRailCommands(editor);
  const closePathState = useClosePathState(editor);
  const documentPanel = useDocumentPanel(editor);
  const [openErrorMessage, setOpenErrorMessage] = useState<string | null>(
    null,
  );
  const [saveErrorMessage, setSaveErrorMessage] = useState<string | null>(
    null,
  );

  useEffect(() => {
    // The file-association launch path runs its open attempt in Tauri's
    // `.setup()`, before this effect's listeners below can possibly have
    // attached yet. A payload from that path has nowhere live to go, so
    // the host buffers it and this is the one-time poll that picks it
    // up.
    invoke<PendingOpenPayload | null>("take_pending_open")
      .then((pending) => {
        if (!pending) {
          return;
        }
        if (pending.kind === "bytes") {
          void openBytes(
            editor,
            pending.path,
            pending.bytes,
            setOpenErrorMessage,
          );
        } else {
          setOpenErrorMessage(pending.message);
        }
      })
      .catch(() => {
        /* Nothing pending is indistinguishable from an unreachable host. */
      });

    const unlistenOpenBytes = listen<OpenBytesPayload>(
      "open-bytes",
      (event) =>
        void openBytes(
          editor,
          event.payload.path,
          event.payload.bytes,
          setOpenErrorMessage,
        ),
    );
    const unlistenOpenError = listen<OpenErrorPayload>(
      "open-error",
      (event) => setOpenErrorMessage(event.payload.message),
    );
    const unlistenNewProject = listen("new-project", () =>
      editor.newProject(),
    );
    const unlistenRequestPack = listen<RequestPackPayload>(
      "request-pack",
      (event) => {
        let bytes: Uint8Array;
        try {
          bytes = editor.packProject(event.payload.app_version);
        } catch {
          // Mirrors the native host's own previous wording for this
          // exact failure, now thrown by the frontend's own `pack`
          // instead of a native `curvyo_document_core::pack`.
          setSaveErrorMessage("This project couldn't be saved.");
          return;
        }
        void invoke("save_project_bytes", {
          bytes: Array.from(bytes),
          saveAs: event.payload.save_as,
        });
      },
    );
    const unlistenSaveError = listen<SaveErrorPayload>(
      "save-error",
      (event) => setSaveErrorMessage(event.payload.message),
    );

    return () => {
      unlistenOpenBytes.then((unlisten) => unlisten());
      unlistenOpenError.then((unlisten) => unlisten());
      unlistenNewProject.then((unlisten) => unlisten());
      unlistenRequestPack.then((unlisten) => unlisten());
      unlistenSaveError.then((unlisten) => unlisten());
    };
    // `editor` is a fresh object every render (new callback identities),
    // but every listener above only ever calls its *current* methods
    // from inside a live event callback — re-subscribing on every
    // render would needlessly tear down and rebuild five listeners for
    // no behavioural difference, so this effect intentionally runs once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden">
      {isTauri ? null : <BrowserBar />}
      <div className="flex min-h-0 flex-1">
        {/* The canvas region, a grid of the rulers' corner square, the two
         * ruler strips and the viewport (`specs/0015-document-size-and-rulers`):
         * the rulers shrink the viewport and are not drawn over it. */}
        <div
          className="grid min-h-0 min-w-0 flex-1"
          style={{
            gridTemplateColumns: `${RULER_THICKNESS_PX}px minmax(0, 1fr)`,
            gridTemplateRows: `${RULER_THICKNESS_PX}px minmax(0, 1fr)`,
          }}
        >
        <Rulers editor={editor} unit={documentPanel.view.unit} />
        {/* The viewport: the rail and the bars' overlay row are anchored to
         * it, not to the window, so no bar is ever drawn under the
         * properties panel on its right (`0007` criterion 39) or over a
         * ruler. */}
        <div className="relative flex min-h-0 min-w-0">
        <ToolRail
          tool={editor.tool}
          selectionCount={editor.selectionCount}
          onSelect={editor.setTool}
          onReturnFocus={() => editor.containerRef.current?.focus()}
          commands={railCommands}
        />
        <Canvas editor={editor} />
        {/* Contextual tool bar: floats over the canvas, right of the tool
         * rail, so showing/hiding it never resizes the canvas
         * (`docs/design-system.md`, "no layout shift on tool switch"). */}
        <div
          className={`pointer-events-none absolute top-3 right-3 left-[calc(var(--rail-right)+12px)] z-20 flex ${
            // The Select bar is left-aligned so its two switches never move
            // when the selection changes; the Node and Shape bars stay
            // centred (`docs/design-system.md`, "Select bar layout").
            editor.tool === "select" ? "justify-start" : "justify-center"
          }`}
        >
          {editor.tool === "select" ? (
            <SelectToolbar
              scaleStrokeWidth={editor.scaleStrokeWidth}
              onSetScaleStrokeWidth={editor.setScaleStrokeWidth}
              scaleCornerRadius={editor.scaleCornerRadius}
              onSetScaleCornerRadius={editor.setScaleCornerRadius}
              linkCorners={editor.linkCorners}
              onSetLinkCorners={editor.setLinkCorners}
              bar={editor.selectBar}
              onSetRadius={editor.setSelectedRadius}
              onRemoveRounding={editor.removeCornerRounding}
              onSetPointCount={editor.setSelectedPointCount}
              onPreviewRatio={editor.previewSelectedRatio}
              onCommitRatio={editor.commitSelectedRatio}
              onConvertToPaths={editor.convertSelectedToPaths}
              onReturnFocus={() => editor.containerRef.current?.focus()}
            />
          ) : null}
          {editor.tool === "node" ? (
            <NodeToolbar
              closePath={{
                state: closePathState,
                onClose: editor.closePath,
                onReturnFocus: () => editor.containerRef.current?.focus(),
                tool: editor.tool,
                selectionCount: editor.selectionCount,
              }}
              state={editor.nodeToolbarState}
              actions={{
                insertSelected: editor.insertSelected,
                deleteSelected: editor.deleteSelected,
                convertSelected: editor.convertSelected,
                makeLine: editor.makeLine,
                makeCurve: editor.makeCurve,
                joinSelected: editor.joinSelected,
                splitSelected: editor.splitSelected,
              }}
            />
          ) : null}
          {editor.tool === "polygon-star" ? (
            <ShapeToolbar
              tool={editor.tool}
              polyStarMode={editor.polyStarMode}
              polyStarPointCount={editor.polyStarPointCount}
              polyStarRatio={editor.polyStarRatio}
              onSetPolyStarMode={editor.setPolyStarMode}
              onSetPolyStarPointCount={editor.setPolyStarPointCount}
              onSetPolyStarRatio={editor.setPolyStarRatio}
            />
          ) : null}
        </div>
        </div>
        </div>
        <PropertiesPanel editor={editor} document={documentPanel} />
      </div>
      <StatusBar
        cursorText={documentPanel.cursorText(cursorMm.x, cursorMm.y)}
        sizeText={documentPanel.view.sizeText}
        zoomPercent={editor.zoomPercent}
      />
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
