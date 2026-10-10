import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { FormatListRecord, WasmSession } from "@/lib/editorSession";
import { pickFormatsFile, saveFormatsFile, setFormatsAside, storeFormats } from "@/lib/formatsFile";
import type { DocumentPanelApi } from "@/hooks/useDocumentPanel";

export interface FormatRow {
  id: string;
  name: string;
  size: string;
  applyName: string;
  starName: string;
  favourite: boolean;
  canStar: boolean;
  selected: boolean;
  canEdit: boolean;
  editName: string;
  deleteName: string;
  deletePrompt: string;
}

export interface FormatGroup {
  id: string;
  name: string;
  count: string;
  enabled: boolean;
  canToggle: boolean;
  canEdit: boolean;
  showName: string;
  foldName: string;
  editName: string;
  deleteName: string;
  deletePrompt: string;
  orientation: "portrait" | "landscape";
  rows: FormatRow[];
}

/** What the "All formats" list shows (`curvyo-ui-core::format_list_view`). */
export interface FormatListView {
  total: string;
  canAdd: boolean;
  canExport: boolean;
  /** Why the user file could not be read; empty while it can. */
  broken: string;
  groups: FormatGroup[];
}

const EMPTY: FormatListView = { total: "", canAdd: false, canExport: false, broken: "", groups: [] };

function readList(session: WasmSession | null): FormatListView {
  if (!session) {
    return EMPTY;
  }
  const raw: FormatListRecord = session.format_list();
  const groupFlags = raw.group_flags;
  const rowFlags = raw.row_flags;
  const rowGroup = raw.row_group;
  const groups: FormatGroup[] = Array.from(raw.group_ids, (id, index) => ({
    id,
    name: raw.group_names[index],
    count: raw.group_counts[index],
    enabled: (groupFlags[index] & 1) !== 0,
    canToggle: (groupFlags[index] & 2) !== 0,
    canEdit: (groupFlags[index] & 4) !== 0,
    showName: raw.group_show_names[index],
    foldName: raw.group_fold_names[index],
    editName: raw.group_edit_names[index],
    deleteName: raw.group_delete_names[index],
    deletePrompt: raw.group_delete_prompts[index],
    orientation: raw.group_orientations[index] as "portrait" | "landscape",
    rows: [],
  }));
  raw.row_ids.forEach((id, index) => {
    groups[rowGroup[index]].rows.push({
      id,
      name: raw.row_names[index],
      size: raw.row_sizes[index],
      applyName: raw.row_apply_names[index],
      starName: raw.row_star_names[index],
      favourite: (rowFlags[index] & 1) !== 0,
      canStar: (rowFlags[index] & 2) !== 0,
      selected: (rowFlags[index] & 4) !== 0,
      canEdit: (rowFlags[index] & 8) !== 0,
      editName: raw.row_edit_names[index],
      deleteName: raw.row_delete_names[index],
      deletePrompt: raw.row_delete_prompts[index],
    });
  });
  const view: FormatListView = {
    total: raw.total_text,
    canAdd: raw.can_add,
    canExport: raw.can_export,
    broken: raw.broken,
    groups,
  };
  raw.free();
  return view;
}

export type FormUnit = "mm" | "cm" | "in" | "px";
export type FormField = "name" | "width" | "height" | "group-name";

/** The typed text of the add and edit form. The host holds it; Rust checks it. */
export interface FormValues {
  name: string;
  width: string;
  height: string;
  unit: FormUnit;
  /** The chosen group's id; empty for "New group...". */
  groupId: string;
  newGroupName: string;
  orientation: "portrait" | "landscape";
  favourite: boolean;
}

export interface OpenForm {
  /** The id of the format being edited; `null` for Add. */
  editing: string | null;
  values: FormValues;
  /** The chip text of every refused field. */
  errors: Partial<Record<FormField, string>>;
  /** The field whose chip shows: the first refused, then the one focused. */
  chip: FormField | null;
  /** Counts up so a refused press moves focus even to the same field. */
  focusRequest: { field: FormField; n: number } | null;
}

export interface GroupForm {
  id: string;
  name: string;
  orientation: "portrait" | "landscape";
  error: string;
}

/** A notice 4 px under the button that was pressed. */
export interface FormatNotice {
  id: number;
  anchor: "add" | "import" | "export" | "broken";
  kind: "success" | "refusal";
  text: string;
}

const SUCCESS_MS = 3000;
const IMPORT_MS = 5000;
const REFUSAL_MS = 8000;

export interface FormatsApi {
  list: FormatListView;
  expanded: boolean;
  toggleExpanded: () => void;
  folded: ReadonlySet<string>;
  toggleFold: (groupId: string) => void;
  pick: (id: string) => void;
  setFavourite: (id: string, on: boolean) => void;
  setGroupEnabled: (id: string, on: boolean) => void;
  form: OpenForm | null;
  openAdd: () => void;
  openEdit: (id: string) => void;
  changeForm: (change: Partial<FormValues>) => void;
  focusFormField: (field: FormField) => void;
  submitForm: () => void;
  cancelForm: () => void;
  groupForm: GroupForm | null;
  openGroupEdit: (id: string) => void;
  changeGroupForm: (change: Partial<GroupForm>) => void;
  saveGroupForm: () => void;
  cancelGroupForm: () => void;
  /** The format or group whose delete is being confirmed. */
  confirm: { kind: "format" | "group"; id: string } | null;
  askDelete: (kind: "format" | "group", id: string) => void;
  cancelDelete: () => void;
  doDelete: () => void;
  importFile: () => void;
  exportFile: () => void;
  setAside: () => void;
  notice: FormatNotice | null;
  /** After a form was saved: where the keyboard goes back to (the row's apply
   * button, or the Add format button when `rowId` is `null`). */
  focusReturn: { rowId: string | null; n: number } | null;
}

const FIELD_ORDER: FormField[] = ["name", "width", "height", "group-name"];

/**
 * The state of the formats library in the Document tab (`specs/0045-document-
 * formats-library/` criteria 12 to 25): whether the list is open, which groups
 * are folded, the open form with its typed text, the delete being confirmed and
 * the notice. Every rule, text and check is the session's; this holds typed
 * text and what is open. It lives above the tab body, so an open form's draft
 * survives a change of tab (`0043` criterion 17) and is dropped when a new
 * session replaces the old one (New, Open).
 */
export function useFormats(
  getSession: () => WasmSession | null,
  doc: DocumentPanelApi,
): FormatsApi {
  const session = getSession();
  const { refresh } = doc;
  // The list is re-read whenever the panel's view is (`doc.view` changes after every
  // sync and every command).
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const list = useMemo(() => readList(session), [session, doc.view]);
  const [expanded, setExpanded] = useState(false);
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());
  const [form, setForm] = useState<OpenForm | null>(null);
  const [groupForm, setGroupForm] = useState<GroupForm | null>(null);
  const [confirm, setConfirm] = useState<FormatsApi["confirm"]>(null);
  const [notice, setNotice] = useState<FormatNotice | null>(null);
  const [focusReturn, setFocusReturn] = useState<FormatsApi["focusReturn"]>(null);
  const lastGroup = useRef("");
  const counter = useRef(0);

  // A new session (New, Open) drops the drafts, not the fold state.
  const [seen, setSeen] = useState(session);
  if (seen !== session) {
    setSeen(session);
    setForm(null);
    setGroupForm(null);
    setConfirm(null);
    setNotice(null);
  }

  useEffect(() => {
    if (!notice) {
      return;
    }
    const ms = notice.kind === "refusal" ? REFUSAL_MS : notice.anchor === "import" ? IMPORT_MS : SUCCESS_MS;
    const handle = window.setTimeout(() => setNotice(null), ms);
    return () => window.clearTimeout(handle);
  }, [notice]);

  const say = useCallback(
    (anchor: FormatNotice["anchor"], kind: FormatNotice["kind"], text: string) => {
      if (text === "") {
        return;
      }
      counter.current += 1;
      setNotice({ id: counter.current, anchor, kind, text });
    },
    [],
  );

  /** Hands the new text of the user file to the host. */
  const write = useCallback(
    async (fileText: string, anchor: FormatNotice["anchor"]) => {
      const failure = await storeFormats(fileText);
      if (failure !== null) {
        say(anchor, "refusal", `Could not save your formats: ${failure}.`);
      }
    },
    [say],
  );

  const toggleExpanded = useCallback(() => setExpanded((open) => !open), []);
  const toggleFold = useCallback(
    (groupId: string) =>
      setFolded((now) => {
        const next = new Set(now);
        if (!next.delete(groupId)) {
          next.add(groupId);
        }
        return next;
      }),
    [],
  );

  const pick = useCallback((id: string) => doc.pickPreset(id), [doc]);

  const setFavourite = useCallback(
    (id: string, on: boolean) => {
      const edit = getSession()?.set_format_favourite(id, on);
      if (edit?.ok) {
        void write(edit.file_text, "add");
      }
      refresh();
    },
    [getSession, refresh, write],
  );

  const setGroupEnabled = useCallback(
    (id: string, on: boolean) => {
      const edit = getSession()?.set_format_group_enabled(id, on);
      if (edit?.ok) {
        void write(edit.file_text, "add");
      }
      refresh();
    },
    [getSession, refresh, write],
  );

  const openWith = useCallback(
    (editing: string | null) => {
      const prefill = getSession()?.format_prefill(editing ?? "", lastGroup.current);
      if (!prefill) {
        return;
      }
      setConfirm(null);
      setGroupForm(null);
      setForm({
        editing,
        values: {
          name: prefill.name,
          width: prefill.width,
          height: prefill.height,
          unit: prefill.unit as FormUnit,
          groupId: prefill.group_id,
          newGroupName: "",
          orientation: "portrait",
          favourite: prefill.favourite,
        },
        errors: {},
        chip: null,
        focusRequest: { field: "name", n: ++counter.current },
      });
    },
    [getSession],
  );

  const changeForm = useCallback((change: Partial<FormValues>) => {
    setForm((now) => {
      if (!now) {
        return now;
      }
      // The next keystroke clears the chip of the field that changed.
      const errors = { ...now.errors };
      if ("name" in change) delete errors.name;
      if ("width" in change || "unit" in change) delete errors.width;
      if ("height" in change || "unit" in change) delete errors.height;
      if ("newGroupName" in change) delete errors["group-name"];
      return { ...now, values: { ...now.values, ...change }, errors, chip: now.chip };
    });
  }, []);

  const focusFormField = useCallback((field: FormField) => {
    setForm((now) => (now ? { ...now, chip: now.errors[field] ? field : null } : now));
  }, []);

  const submitForm = useCallback(() => {
    const current = form;
    const active = getSession();
    if (!current || !active) {
      return;
    }
    const { values } = current;
    const fields = [
      values.name,
      values.width,
      values.height,
      values.unit,
      values.groupId,
      values.newGroupName,
      values.orientation,
    ];
    const edit =
      current.editing === null
        ? active.add_format(fields, values.favourite)
        : active.save_format(current.editing, fields, values.favourite);
    if (!edit.ok) {
      const errors: Partial<Record<FormField, string>> = {};
      edit.error_fields.forEach((field, index) => {
        errors[field as FormField] = edit.error_messages[index];
      });
      const first = FIELD_ORDER.find((field) => errors[field] !== undefined) ?? null;
      setForm({
        ...current,
        errors,
        chip: first,
        focusRequest: first ? { field: first, n: ++counter.current } : null,
      });
      return;
    }
    lastGroup.current = values.groupId;
    setForm(null);
    setFocusReturn({ rowId: current.editing, n: ++counter.current });
    void write(edit.file_text, "add");
    say("add", "success", edit.notice);
    refresh();
  }, [form, getSession, refresh, say, write]);

  const cancelForm = useCallback(() => setForm(null), []);

  const openGroupEdit = useCallback(
    (id: string) => {
      const group = list.groups.find((g) => g.id === id);
      if (!group) {
        return;
      }
      setForm(null);
      setConfirm(null);
      setGroupForm({ id, name: group.name, orientation: group.orientation, error: "" });
    },
    [list],
  );

  const changeGroupForm = useCallback(
    (change: Partial<GroupForm>) =>
      setGroupForm((now) => (now ? { ...now, ...change, error: "" } : now)),
    [],
  );

  const saveGroupForm = useCallback(() => {
    const active = getSession();
    if (!groupForm || !active) {
      return;
    }
    const edit = active.save_format_group(groupForm.id, groupForm.name, groupForm.orientation);
    if (!edit.ok) {
      setGroupForm({ ...groupForm, error: edit.error_messages[0] ?? "" });
      return;
    }
    setGroupForm(null);
    void write(edit.file_text, "add");
    refresh();
  }, [groupForm, getSession, refresh, write]);

  const cancelGroupForm = useCallback(() => setGroupForm(null), []);

  const askDelete = useCallback((kind: "format" | "group", id: string) => {
    setForm(null);
    setGroupForm(null);
    setConfirm({ kind, id });
  }, []);
  const cancelDelete = useCallback(() => setConfirm(null), []);

  const doDelete = useCallback(() => {
    const active = getSession();
    if (!confirm || !active) {
      return;
    }
    const edit =
      confirm.kind === "format"
        ? active.delete_format(confirm.id)
        : active.delete_format_group(confirm.id);
    setConfirm(null);
    if (edit.ok) {
      void write(edit.file_text, "add");
    }
    refresh();
  }, [confirm, getSession, refresh, write]);

  const importFile = useCallback(() => {
    void (async () => {
      let picked;
      try {
        picked = await pickFormatsFile();
      } catch (error) {
        say("import", "refusal", `Could not import: ${error instanceof Error ? error.message : String(error)}.`);
        return;
      }
      const active = getSession();
      if (!picked || !active) {
        return;
      }
      const edit = active.import_formats(picked.text, picked.name);
      if (edit.ok) {
        await write(edit.file_text, "import");
      }
      say("import", edit.ok ? "success" : "refusal", edit.notice);
      refresh();
    })();
  }, [getSession, refresh, say, write]);

  const exportFile = useCallback(() => {
    void (async () => {
      const text = getSession()?.export_formats() ?? "";
      if (text === "") {
        return;
      }
      try {
        await saveFormatsFile(text);
      } catch (error) {
        say("export", "refusal", `Could not export: ${error instanceof Error ? error.message : String(error)}.`);
      }
    })();
  }, [getSession, say]);

  const setAside = useCallback(() => {
    void (async () => {
      const failure = await setFormatsAside();
      if (failure !== null) {
        say("broken", "refusal", `Could not move the file: ${failure}.`);
        return;
      }
      getSession()?.formats_file_set_aside();
      say("broken", "success", "Moved to document-formats.toml.broken.");
      refresh();
    })();
  }, [getSession, refresh, say]);

  return {
    list,
    expanded,
    toggleExpanded,
    folded,
    toggleFold,
    pick,
    setFavourite,
    setGroupEnabled,
    form,
    openAdd: () => openWith(null),
    openEdit: openWith,
    changeForm,
    focusFormField,
    submitForm,
    cancelForm,
    groupForm,
    openGroupEdit,
    changeGroupForm,
    saveGroupForm,
    cancelGroupForm,
    confirm,
    askDelete,
    cancelDelete,
    doDelete,
    importFile,
    exportFile,
    setAside,
    notice,
    focusReturn,
  };
}
