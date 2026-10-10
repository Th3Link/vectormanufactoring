import { CircleAlert } from "lucide-react";

import { OUTLINE_BUTTON } from "@/components/panel/FormatForm";
import type { FormatsApi } from "@/hooks/useFormats";

/**
 * The block at the top of the format area while the user file cannot be read
 * (`specs/0045-document-formats-library/` criterion 6): the reason, the
 * built-in formats shown, and "Set file aside". Persistent, not a live region.
 * After the file is set aside the block leaves and the notice shows for 3
 * seconds.
 */
export function FormatsBrokenBlock({ formats }: { formats: FormatsApi }) {
  const { broken } = formats.list;
  const notice = formats.notice?.anchor === "broken" ? formats.notice : null;
  if (broken === "") {
    return notice ? (
      <p role="status" className="rounded-md bg-[var(--toolbar-bg)] px-2 py-1 text-xs text-[var(--toolbar-icon)]">
        {notice.text}
      </p>
    ) : null;
  }
  return (
    <div className="flex flex-col gap-2 rounded-[5px] border border-[var(--field-invalid)] p-2">
      <p className="flex items-start gap-1 text-xs text-[var(--field-invalid)]">
        <CircleAlert size={12} aria-hidden className="mt-0.5 shrink-0" />
        <span>Your formats file could not be read: {broken}. Built-in formats are shown.</span>
      </p>
      <button type="button" onClick={formats.setAside} className={`${OUTLINE_BUTTON} w-full`}>
        Set file aside
      </button>
      {notice?.kind === "refusal" && (
        <p role="alert" className="text-xs text-[var(--field-invalid)]">
          {notice.text}
        </p>
      )}
    </div>
  );
}
