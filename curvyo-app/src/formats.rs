//! The commands that move the text of the maker's formats file
//! (`specs/0045-document-formats-library/`, `adrs.md` decision 5): read it,
//! write it, set it aside, import and export through the native file dialogs.
//! They parse nothing; the library in `curvyo-document-core` does, inside the
//! frontend's `WasmSession`. The file is `document-formats.toml` in the app
//! data directory (ADR 0004 §7), written atomically with no lock file.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

/// The file's name in the data directory.
const FORMATS_FILE: &str = "document-formats.toml";
/// The suffix of a file that could not be read and was set aside.
const BROKEN_SUFFIX: &str = ".broken";
/// The name an export starts with.
const EXPORT_FILE_NAME: &str = "curvyo-formats.toml";
const FORMATS_FILTER_NAME: &str = "Curvyo formats";

/// A file the maker picked for Import.
#[derive(Clone, serde::Serialize)]
pub(crate) struct ImportedFormats {
    /// The file's name, for the refusal message.
    name: String,
    /// Its text; not looked at here.
    text: String,
}

fn formats_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(FORMATS_FILE))
        .map_err(|error| format!("the data folder is not available ({error})"))
}

fn text_of(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| "the file is not valid UTF-8 text".to_string())
}

/// The text of the user file; `None` when there is none yet.
#[tauri::command]
pub(crate) fn read_formats_file(app: AppHandle) -> Result<Option<String>, String> {
    let path = formats_path(&app)?;
    match curvyo_storage_io::read_optional(&path).map_err(|e| e.to_string())? {
        Some(bytes) => text_of(bytes).map(Some),
        None => Ok(None),
    }
}

/// Replaces the user file with `text`, atomically; the folder is created at the
/// first write.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn write_formats_file(app: AppHandle, text: String) -> Result<(), String> {
    let path = formats_path(&app)?;
    curvyo_storage_io::ensure_parent_dir(&path).map_err(|e| e.to_string())?;
    curvyo_storage_io::write_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())
}

/// Renames the user file to `document-formats.toml.broken`, replacing an older
/// one. The file is never changed or deleted otherwise.
#[tauri::command]
pub(crate) fn set_formats_file_aside(app: AppHandle) -> Result<(), String> {
    let path = formats_path(&app)?;
    let mut aside = path.clone().into_os_string();
    aside.push(BROKEN_SUFFIX);
    curvyo_storage_io::rename_replacing(&path, &PathBuf::from(aside)).map_err(|e| e.to_string())
}

/// The native picker for Import. `None` when the maker cancelled. Runs off the
/// main thread, which a blocking dialog needs.
#[tauri::command]
pub(crate) async fn import_formats_file(app: AppHandle) -> Result<Option<ImportedFormats>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter(FORMATS_FILTER_NAME, &["toml"])
        .blocking_pick_file();
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    let bytes = curvyo_storage_io::read_to_vec(&path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    Ok(Some(ImportedFormats {
        name,
        text: text_of(bytes)?,
    }))
}

/// The native Save dialog for Export, then the write. `false` when the maker
/// cancelled.
#[tauri::command]
pub(crate) async fn export_formats_file(app: AppHandle, text: String) -> Result<bool, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter(FORMATS_FILTER_NAME, &["toml"])
        .set_file_name(EXPORT_FILE_NAME)
        .blocking_save_file();
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(false);
    };
    curvyo_storage_io::write_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(true)
}
