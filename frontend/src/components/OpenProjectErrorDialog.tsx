import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

interface OpenProjectErrorDialogProps {
  /** The one-sentence cause, or `null` when there is nothing to show. */
  message: string | null;
  onDismiss: () => void;
}

/**
 * The blocking failure dialog for acceptance criterion 7: a `.vmf` that
 * isn't a zip, is damaged, or is from a newer version of the app.
 *
 * Uses shadcn/ui `AlertDialog` rather than a toast or `Dialog` — it is the
 * primitive that does not dismiss on an outside click, which matters here
 * because this is a failure the maker must acknowledge
 * (specs/project-file-foundation: "Error handling — invalid/corrupt
 * file"). Opening this dialog never depends on replacing the open
 * project's state — the host only emits the error after deciding *not*
 * to touch it.
 */
export function OpenProjectErrorDialog({
  message,
  onDismiss,
}: OpenProjectErrorDialogProps) {
  return (
    <AlertDialog
      open={message !== null}
      onOpenChange={(open) => {
        if (!open) {
          onDismiss();
        }
      }}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Can&apos;t open project</AlertDialogTitle>
          <AlertDialogDescription>{message}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction autoFocus onClick={onDismiss}>
            OK
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
