import { TooltipProvider, Tooltip } from "@/components/ui/tooltip";
import { useActionNotice } from "@/hooks/useActionNotice";
import {
  CLOSE_BUTTONS,
  closeNoticeMs,
  closeNoticeText,
  closeTooltipNote,
  type ClosePathResult,
  type ClosePathState,
} from "@/lib/penText";

interface ClosePathGroupProps {
  state: ClosePathState;
  /** Runs the command with a join; the host syncs the session. */
  onClose: (join: "sharp" | "smooth") => ClosePathResult;
  /** Called after a mouse press, so the tool letters keep working; a key press leaves the focus
   * on the button. */
  onReturnFocus: () => void;
  tool: string;
  selectionCount: number;
}

/**
 * The Close path group at the end of the Node bar (`docs/design-system.md`, row "Close path
 * group"; `specs/0034-pen-path-extension/` criteria 18 to 21): a label and two text buttons,
 * Sharp and Smooth. Commands, not toggles. Dimmed (`aria-disabled`, still focusable, inert, the
 * tooltip says why) when the editing set holds no open path with three or more nodes. The notice
 * of the last press sits 4 px below the pressed button, right-aligned to the group.
 */
export function ClosePathGroup({
  state,
  onClose,
  onReturnFocus,
  tool,
  selectionCount,
}: ClosePathGroupProps) {
  const dimmed = state.closable === 0;
  const { notice, show } = useActionNotice(tool, selectionCount, () => {});
  return (
    <TooltipProvider>
      <div className="relative flex items-center gap-1">
        <div role="group" aria-label="Close path" className="flex items-center gap-1">
          <span className="px-1 text-sm text-[var(--toolbar-icon)]">Close path</span>
          {CLOSE_BUTTONS.map((button) => (
            <Tooltip
              key={button.join}
              side="bottom"
              content={
                <div className="max-w-[260px]">
                  <div className="font-semibold whitespace-nowrap">{button.title}</div>
                  <div className="whitespace-nowrap">{button.rule}</div>
                  <div style={{ color: "var(--panel-muted-fg)" }}>{closeTooltipNote(state)}</div>
                </div>
              }
            >
              <button
                type="button"
                aria-label={button.name}
                aria-disabled={dimmed}
                onClick={(event) => {
                  if (dimmed) {
                    event.preventDefault();
                    return;
                  }
                  const result = onClose(button.join);
                  const text = closeNoticeText(result);
                  if (text !== null) {
                    show("success", text, closeNoticeMs(text));
                  }
                  // `detail` is 0 for keyboard activation, 1 or more for a mouse click.
                  if (event.detail > 0) {
                    onReturnFocus();
                  }
                }}
                className={`h-7 rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] bg-transparent px-3 text-sm text-[var(--toolbar-icon)] outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] ${
                  dimmed ? "opacity-40" : "hover:bg-[var(--editor-accent-hover)]"
                }`}
              >
                {button.label}
              </button>
            </Tooltip>
          ))}
        </div>
        {/* The live region is always in the tree, visually hidden, so its text change is
            announced; the visible notice is a text-only copy. */}
        <p role="status" className="sr-only">
          {notice?.text ?? ""}
        </p>
        {notice && (
          <p
            aria-hidden
            className="pointer-events-none absolute top-full right-0 z-30 mt-1 max-w-[360px] rounded-md bg-[var(--toolbar-bg)] px-2 py-1 text-xs text-[var(--toolbar-icon)]"
            style={{ boxShadow: "var(--panel-elevation-shadow)" }}
          >
            {notice.text}
          </p>
        )}
      </div>
    </TooltipProvider>
  );
}
