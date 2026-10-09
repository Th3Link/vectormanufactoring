//! The Curvyo desktop host: a Tauri 2 binary owning the native menu,
//! native file dialogs, the `.curvyo` file association and all filesystem
//! access, behind the narrow command/event interface this module defines
//! (ADR 0001 §6).
//!
//! `path-node-editing`'s own PR review narrowed this crate's job further:
//! the host does byte I/O only. The actual document lives in the
//! `WasmSession` the frontend owns (`curvyo-editor-wasm`, hosted in the
//! webview); this crate never builds or reads a
//! `curvyo_document_core::Document` itself any more. Save hands this
//! host already-packed bytes (via [`save_project_bytes`]) and it writes
//! them; Open reads bytes from disk and hands them to the frontend (the
//! "open-bytes" event) for `WasmSession::open` to parse.

// `CLAUDE.md` §5 allows unwrap/expect in tests; only production code is held
// to the stricter rule.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod menu;
mod state;

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use state::{AppState, PendingOpen, ProjectState};

/// The main window's label, matching `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// The native file filter/extension for a Curvyo project, used by both
/// dialogs and the file association in `tauri.conf.json`.
const PROJECT_EXTENSION: &str = "curvyo";
const PROJECT_FILTER_NAME: &str = "Curvyo project";
const DEFAULT_FILE_NAME: &str = "Untitled.curvyo";

/// A host-level failure to even read a path's bytes (missing file,
/// permissions, not a file) — distinct from a `.curvyo` whose *content* is
/// invalid, which is the frontend's own refusal once it tries
/// `WasmSession::open` on bytes this host did successfully read.
#[derive(Clone, serde::Serialize)]
struct OpenErrorPayload {
    message: String,
}

/// What the "open-bytes" event (and a successful [`take_pending_open`])
/// carries: a file this host *could* read, for the frontend to try
/// `WasmSession::open` on — this host never looks inside these bytes
/// itself.
#[derive(Clone, serde::Serialize)]
struct OpenBytesPayload {
    path: String,
    bytes: Vec<u8>,
}

/// The discriminated union [`take_pending_open`] returns, mirroring
/// [`state::PendingOpen`] across the IPC boundary.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind")]
enum PendingOpenPayload {
    #[serde(rename = "bytes")]
    Bytes { path: String, bytes: Vec<u8> },
    #[serde(rename = "io_error")]
    IoError { message: String },
}

impl From<PendingOpen> for PendingOpenPayload {
    fn from(pending: PendingOpen) -> Self {
        match pending {
            PendingOpen::Bytes { path, bytes } => Self::Bytes {
                path: path.to_string_lossy().into_owned(),
                bytes,
            },
            PendingOpen::IoError(message) => Self::IoError { message },
        }
    }
}

/// A save failure (File → Save / Save As). Unlike [`OpenErrorPayload`]
/// this has no three-case taxonomy — "the write failed" is the whole
/// story the UI needs — but it must still reach the user: a save that
/// silently fails and looks successful is the worst failure mode this
/// product can have.
#[derive(Clone, serde::Serialize)]
struct SaveErrorPayload {
    message: String,
}

/// What the "request-pack" event carries: the frontend packs the live
/// session's document (`WasmSession::pack(app_version)`) and hands the
/// resulting bytes to [`save_project_bytes`] — this host decides *when*
/// (Save vs Save As) but never builds the bytes itself.
#[derive(Clone, serde::Serialize)]
struct RequestPackPayload {
    save_as: bool,
    app_version: String,
}

/// Returns and clears any open payload buffered before the frontend had
/// mounted — the file-association launch path's open attempt runs in
/// `.setup()`, ahead of any `listen()` call. Call once, at mount.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn take_pending_open(state: State<AppState>) -> Option<PendingOpenPayload> {
    state.take_pending_open().map(PendingOpenPayload::from)
}

/// The frontend's confirmation that bytes this host handed it (via
/// "open-bytes" or [`take_pending_open`]) parsed successfully and are now
/// the live `WasmSession`. Only now does this host start treating `path`
/// as the open project's own path — acceptance criterion 7: a refusal
/// (the frontend never calling this) must never touch an already-open
/// project, and nothing here ever does.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn confirm_project_opened(app: AppHandle, state: State<AppState>, path: String) {
    let mut project = state.lock_project();
    *project = ProjectState::opened(PathBuf::from(path));
    let file_name = project.file_name();
    drop(project);
    set_window_title(&app, file_name.as_deref());
}

/// The frontend's half of Save/Save As: here are the live session's
/// already-packed bytes, write them. `save_as` forces the native save
/// dialog even when a path is already known (acceptance criterion 3);
/// otherwise an unknown path falls back to the same dialog (first-ever
/// save).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn save_project_bytes(app: AppHandle, state: State<AppState>, bytes: Vec<u8>, save_as: bool) {
    let existing_path = if save_as {
        None
    } else {
        state.lock_project().path.clone()
    };

    let Some(path) = existing_path.or_else(|| pick_save_path(&app)) else {
        return; // no known path, and the user cancelled the dialog.
    };

    if curvyo_storage_io::write_atomic(&path, &bytes).is_err() {
        // A failed save that looks successful is the worst failure mode
        // this product can have — report it, don't just return.
        emit_save_error(
            &app,
            "This project couldn't be saved. Check that you have permission to write to this location and that there's enough disk space, then try again.",
        );
        return;
    }

    let mut project = state.lock_project();
    project.path = Some(path);
    let file_name = project.file_name();
    drop(project);
    set_window_title(&app, file_name.as_deref());
}

/// Whether `WEBKIT_DISABLE_DMABUF_RENDERER` still needs to be set in this
/// process's own environment. Compiled on Linux (where `main` acts on a
/// `true` result, since only `WebKitGTK` has the bug this variable works
/// around) and under `#[cfg(test)]` on every platform, so the pure
/// environment-reading logic stays plainly testable without needing a
/// Linux runner — it would otherwise be genuinely dead code in a
/// non-Linux, non-test build, since its only production caller is itself
/// Linux-only.
#[cfg(any(target_os = "linux", test))]
fn webkit_dmabuf_env_var_is_unset() -> bool {
    std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
}

/// Re-executes this same binary, replacing the current process image,
/// with `WEBKIT_DISABLE_DMABUF_RENDERER=1` set in the *child's*
/// environment via [`std::process::Command::env`] — a safe API, unlike
/// mutating the current process's own environment with
/// `std::env::set_var` (`unsafe` as of this edition, and `CLAUDE.md` §5
/// forbids new `unsafe` outside `*-core` crates without an ADR). Linux
/// only, before any webview is created.
///
/// `specs/0002-path-node-editing/adrs.md`'s canvas-perf spike measured that,
/// on this stack (`WebKitGTK` 2.52.6 via `wry`/`tao`, NVIDIA proprietary
/// driver), leaving this unset acquires a `WebGL2` context that reports
/// success but renders **zero frames** — for `wgpu` and for hand-written
/// `WebGL2` alike. The failure mode is a silent blank canvas, not an
/// error, which is exactly why this is a tested startup step rather than
/// a hope.
///
/// Never returns on success: the process image is replaced in place, so
/// the relaunched process continues exactly where this one left off
/// (same argv, same working directory), just with the variable now set.
/// Panics if the relaunch itself could not even be started — there is no
/// sane fallback, since continuing would silently reproduce the blank-
/// canvas bug this function exists to prevent.
#[cfg(target_os = "linux")]
fn relaunch_for_linux_webkit_env() -> ! {
    use std::os::unix::process::CommandExt;

    // invariant: a running process always has a resolvable path to its
    // own executable.
    #[allow(clippy::expect_used)]
    let exe = std::env::current_exe().expect("resolve the running executable's own path");
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let error = std::process::Command::new(exe)
        .args(args)
        .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        .exec();
    panic!("failed to relaunch with WEBKIT_DISABLE_DMABUF_RENDERER set: {error}");
}

// invariant: `.build()` only returns an `Err` for a host-level setup
// failure (e.g. the webview engine missing) that leaves the process with
// nothing useful to do; there is no caller to propagate a `Result` to
// from `main`.
#[allow(clippy::expect_used)]
fn main() {
    #[cfg(target_os = "linux")]
    if webkit_dmabuf_env_var_is_unset() {
        relaunch_for_linux_webkit_env();
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            take_pending_open,
            confirm_project_opened,
            save_project_bytes,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let menu = menu::build(app)?;
            app.set_menu(menu)?;
            let menu_handle = handle.clone();
            app.on_menu_event(move |_app, event| {
                menu::dispatch(&menu_handle, event.id().as_ref());
            });

            // A `.curvyo` opened via the OS file association on Linux/Windows
            // arrives as argv[1] (specs/0001-project-file-foundation, AC5).
            if let Some(path) = std::env::args().nth(1) {
                let path = PathBuf::from(path);
                if path.extension().and_then(|ext| ext.to_str()) == Some(PROJECT_EXTENSION) {
                    open_path(&handle, &path);
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Curvyo application");

    // macOS (and iOS) deliver a double-clicked `.curvyo` as an Apple Event,
    // surfaced here rather than as argv[1] — the file-association half of
    // acceptance criterion 5 that the Linux/Windows argv path above does
    // not cover. Not build-verified on real macOS hardware in this
    // sandbox; code-complete pending that verification.
    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = event {
            for url in urls {
                if let Ok(path) = url.to_file_path()
                    && path.extension().and_then(|ext| ext.to_str()) == Some(PROJECT_EXTENSION)
                {
                    open_path(app_handle, &path);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app_handle, event);
        }
    });
}

fn window_title(file_name: Option<&str>) -> String {
    match file_name {
        Some(name) => format!("{name} — Curvyo"),
        None => "Curvyo".to_string(),
    }
}

fn set_window_title(app: &AppHandle, file_name: Option<&str>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.set_title(&window_title(file_name));
    }
}

/// Emits the live "open-error" event (for an already-mounted frontend) and
/// buffers the same message in [`AppState`] (for the file-association
/// launch race — see [`take_pending_open`]). Harmless if nobody ever
/// reads the buffered copy: it is only polled once, at mount.
fn emit_open_error(app: &AppHandle, message: String) {
    app.state::<AppState>()
        .set_pending_open(PendingOpen::IoError(message.clone()));
    let _ = app.emit("open-error", OpenErrorPayload { message });
}

/// Reports a failed Save/Save As. There is always a running, mounted
/// frontend by the time a save can happen (unlike open, it is never
/// triggered from `.setup()`), so — unlike open-error — this has no
/// pending-buffer fallback to worry about.
fn emit_save_error(app: &AppHandle, message: &str) {
    let payload = SaveErrorPayload {
        message: message.to_string(),
    };
    let _ = app.emit("save-error", payload);
}

/// Handles File → New: a fresh, never-saved project replaces whatever was
/// open (acceptance criterion 2). The frontend reacts to "new-project" by
/// swapping in a brand-new, empty `WasmSession` of its own.
pub(crate) fn handle_new(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut project = state.lock_project();
    *project = ProjectState::new_unsaved();
    drop(project);
    set_window_title(app, None);
    let _ = app.emit("new-project", ());
}

/// Handles File → Open…: the native picker, then [`open_path`].
pub(crate) fn handle_open(app: &AppHandle) {
    let picked = app
        .dialog()
        .file()
        .add_filter(PROJECT_FILTER_NAME, &[PROJECT_EXTENSION])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return; // user cancelled; already-open project stays untouched.
    };
    let Some(path) = file_path.into_path().ok() else {
        return;
    };
    open_path(app, &path);
}

/// Reads the bytes at `path` and hands them to the frontend to open
/// (acceptance criterion 7: a refusal never touches an already-open
/// project — this host does not update [`AppState`]'s path at all here;
/// [`confirm_project_opened`] does, once the frontend reports success).
fn open_path(app: &AppHandle, path: &Path) {
    let Ok(bytes) = curvyo_storage_io::read_to_vec(path) else {
        // Not one of curvyo_document_core::OpenError's three cases (that
        // taxonomy is about file *content*, and only the frontend's
        // `WasmSession::open` ever sees it now), but the UI has one
        // failure dialog, so an unreadable path is reported the same way
        // a damaged container used to be.
        emit_open_error(app, "This file is damaged and can't be read.".to_string());
        return;
    };
    app.state::<AppState>()
        .set_pending_open(PendingOpen::Bytes {
            path: path.to_path_buf(),
            bytes: bytes.clone(),
        });
    let _ = app.emit(
        "open-bytes",
        OpenBytesPayload {
            path: path.to_string_lossy().into_owned(),
            bytes,
        },
    );
}

/// Handles File → Save: asks the frontend to pack the live session and
/// hand this host the bytes (via [`save_project_bytes`]), writing to the
/// project's known path once it does, or falling back to the Save As
/// dialog when the project has never been saved.
pub(crate) fn handle_save(app: &AppHandle) {
    request_pack(app, false);
}

/// Handles File → Save As…: always prompts, defaulting to `Untitled.curvyo`
/// (acceptance criterion 3).
pub(crate) fn handle_save_as(app: &AppHandle) {
    request_pack(app, true);
}

/// The native Save As… dialog, defaulting to `Untitled.curvyo` (acceptance
/// criterion 3). Shared by [`save_project_bytes`]'s "no known path yet"
/// and "Save As was explicitly requested" cases.
fn pick_save_path(app: &AppHandle) -> Option<PathBuf> {
    app.dialog()
        .file()
        .add_filter(PROJECT_FILTER_NAME, &[PROJECT_EXTENSION])
        .set_file_name(DEFAULT_FILE_NAME)
        .blocking_save_file()
        .and_then(|file_path| file_path.into_path().ok())
}

fn request_pack(app: &AppHandle, save_as: bool) {
    let _ = app.emit(
        "request-pack",
        RequestPackPayload {
            save_as,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    );
}

#[cfg(test)]
mod webkit_dmabuf_env_var_tests {
    use super::webkit_dmabuf_env_var_is_unset;

    /// `specs/0002-path-node-editing/adrs.md`'s prerequisite note: the failure
    /// mode of leaving `WEBKIT_DISABLE_DMABUF_RENDERER` unset is a silent
    /// blank canvas, not an error, so this needs a test rather than a
    /// hope. This only pins the detection half (reading an environment
    /// variable is safe); the relaunch itself
    /// (`relaunch_for_linux_webkit_env`) replaces the current process
    /// image on success and is exercised by running the built binary, not
    /// by a unit test. Nothing else in this crate's test suite touches
    /// this specific variable.
    #[test]
    fn is_unset_by_default_in_a_fresh_test_process() {
        assert!(
            webkit_dmabuf_env_var_is_unset(),
            "WebKitGTK's DMA-BUF renderer must be disabled before any webview is created; \
             this build does that by relaunching itself with the variable set, which this \
             check decides is necessary"
        );
    }
}
