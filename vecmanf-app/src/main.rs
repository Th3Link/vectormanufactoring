//! The vecmanf desktop host: a Tauri 2 binary owning the native menu,
//! native file dialogs, the `.vmf` file association and all filesystem
//! access, behind the narrow command/event interface this module defines
//! (ADR 0001 §6). No editing logic lives here — this slice draws nothing,
//! so there is no wasm editor core to host yet
//! (`specs/project-file-foundation/adrs.md`).

// `CLAUDE.md` §5 allows unwrap/expect in tests; only production code is held
// to the stricter rule.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod menu;
mod open_error;
mod state;

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use open_error::map_open_error;
use state::{AppState, ProjectState};

/// The main window's label, matching `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// The native file filter/extension for a vecmanf project, used by both
/// dialogs and the file association in `tauri.conf.json`.
const VMF_EXTENSION: &str = "vmf";
const VMF_FILTER_NAME: &str = "vecmanf project";
const DEFAULT_FILE_NAME: &str = "Untitled.vmf";

/// The project state pushed to the frontend after New, Open, Save or Save
/// As succeeds — just enough for the status bar (ADR 0002 §2's mm size).
/// Title bar text is owned entirely by this host, not the frontend
/// (native window title, ADR 0001 §1).
#[derive(Clone, serde::Serialize)]
struct ProjectStatePayload {
    size_mm: SizeMmPayload,
}

#[derive(Clone, serde::Serialize)]
struct SizeMmPayload {
    width: f64,
    height: f64,
}

impl From<vecmanf_document_core::DocumentSize> for SizeMmPayload {
    fn from(size: vecmanf_document_core::DocumentSize) -> Self {
        Self {
            width: size.width.as_mm(),
            height: size.height.as_mm(),
        }
    }
}

/// The three named refusal reasons of acceptance criterion 7, already
/// mapped to user-facing English by [`map_open_error`].
#[derive(Clone, serde::Serialize)]
struct OpenErrorPayload {
    message: String,
}

/// Returns the current project's state, for the frontend to read once on
/// mount (avoids a race with an event emitted before any listener is
/// attached).
#[tauri::command]
// Tauri's command extractor requires `State<'_, T>` to be taken by value;
// it is a cheap handle, not an owned value that goes unused.
#[allow(clippy::needless_pass_by_value)]
fn get_project_state(state: State<AppState>) -> ProjectStatePayload {
    let project = state.lock_project();
    ProjectStatePayload {
        size_mm: project.document.size().into(),
    }
}

// invariant: `.run()` only returns an `Err` for a host-level setup failure
// (e.g. the webview engine missing) that leaves the process with nothing
// useful to do; there is no caller to propagate a `Result` to from `main`.
#[allow(clippy::expect_used)]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![get_project_state])
        .setup(|app| {
            let handle = app.handle().clone();
            let menu = menu::build(app)?;
            app.set_menu(menu)?;
            let menu_handle = handle.clone();
            app.on_menu_event(move |_app, event| {
                menu::dispatch(&menu_handle, event.id().as_ref());
            });

            // A `.vmf` opened via the OS file association on Linux/Windows
            // arrives as argv[1] (specs/project-file-foundation, AC5).
            if let Some(path) = std::env::args().nth(1) {
                let path = PathBuf::from(path);
                if path.extension().and_then(|ext| ext.to_str()) == Some(VMF_EXTENSION) {
                    open_path(&handle, &path);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the vecmanf application");
}

fn window_title(file_name: Option<&str>) -> String {
    match file_name {
        Some(name) => format!("{name} — vecmanf"),
        None => "vecmanf".to_string(),
    }
}

fn set_window_title(app: &AppHandle, file_name: Option<&str>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.set_title(&window_title(file_name));
    }
}

fn emit_project_state(app: &AppHandle, project: &ProjectState) {
    let payload = ProjectStatePayload {
        size_mm: project.document.size().into(),
    };
    let _ = app.emit("project-state", payload);
}

fn emit_open_error(app: &AppHandle, error: &vecmanf_document_core::OpenError) {
    let payload = OpenErrorPayload {
        message: map_open_error(error),
    };
    let _ = app.emit("open-error", payload);
}

/// Handles File → New: a fresh, never-saved document replaces whatever was
/// open (acceptance criterion 2).
pub(crate) fn handle_new(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut project = state.lock_project();
    *project = ProjectState::new_unsaved();
    set_window_title(app, None);
    emit_project_state(app, &project);
}

/// Handles File → Open…: the native picker, then [`open_path`].
pub(crate) fn handle_open(app: &AppHandle) {
    let picked = app
        .dialog()
        .file()
        .add_filter(VMF_FILTER_NAME, &[VMF_EXTENSION])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return; // user cancelled; already-open project stays untouched.
    };
    let Some(path) = file_path.into_path().ok() else {
        return;
    };
    open_path(app, &path);
}

/// Opens the `.vmf` project at `path`, replacing the current project only
/// on success (acceptance criterion 7: a refusal never touches an
/// already-open project).
fn open_path(app: &AppHandle, path: &Path) {
    let Ok(bytes) = vecmanf_storage_io::read_to_vec(path) else {
        // Not one of vecmanf_document_core::OpenError's three cases (that
        // taxonomy is about file *contents*), but the UI has one failure
        // dialog, so an unreadable path is reported the same way as a
        // damaged container.
        emit_open_error(app, &vecmanf_document_core::OpenError::Damaged);
        return;
    };

    match vecmanf_document_core::unpack(&bytes) {
        Ok(document) => {
            let state = app.state::<AppState>();
            let mut project = state.lock_project();
            *project = ProjectState::opened(document, path.to_path_buf());
            let file_name = project.file_name();
            set_window_title(app, file_name.as_deref());
            emit_project_state(app, &project);
        }
        Err(error) => emit_open_error(app, &error),
    }
}

/// Handles File → Save: writes to the project's known path, or falls back
/// to Save As when the project has never been saved.
pub(crate) fn handle_save(app: &AppHandle) {
    let existing_path = {
        let state = app.state::<AppState>();
        let project = state.lock_project();
        project.path.clone()
    };

    match existing_path {
        Some(path) => save_to(app, &path),
        None => handle_save_as(app),
    }
}

/// Handles File → Save As…: always prompts, defaulting to `Untitled.vmf`
/// (acceptance criterion 3).
pub(crate) fn handle_save_as(app: &AppHandle) {
    let picked = app
        .dialog()
        .file()
        .add_filter(VMF_FILTER_NAME, &[VMF_EXTENSION])
        .set_file_name(DEFAULT_FILE_NAME)
        .blocking_save_file();

    let Some(file_path) = picked else {
        return; // user cancelled.
    };
    let Some(path) = file_path.into_path().ok() else {
        return;
    };
    save_to(app, &path);
}

fn save_to(app: &AppHandle, path: &Path) {
    let state = app.state::<AppState>();
    let mut project = state.lock_project();

    let Ok(bytes) = vecmanf_document_core::pack(&project.document, env!("CARGO_PKG_VERSION"))
    else {
        return; // nothing user-actionable to report for this slice.
    };
    if vecmanf_storage_io::write_atomic(path, &bytes).is_err() {
        return;
    }

    project.path = Some(path.to_path_buf());
    let file_name = project.file_name();
    set_window_title(app, file_name.as_deref());
    emit_project_state(app, &project);
}
