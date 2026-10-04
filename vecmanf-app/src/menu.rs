//! The native File menu: New, Open…, Save, Save As…, a separator, Quit —
//! the one top-level menu this slice adds, with OS-standard accelerators
//! bound directly on the menu items
//! (`specs/0001-project-file-foundation/specification.md`, "Keyboard shortcuts
//! (from day one)"). Native menus and accelerators are free accessibility
//! and keyboard traversal on every platform (same spec, "Window chrome and
//! menu").

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Wry};

const NEW_ID: &str = "new";
const OPEN_ID: &str = "open";
const SAVE_ID: &str = "save";
const SAVE_AS_ID: &str = "save_as";

/// Builds the application menu (just the File menu, for this slice).
///
/// # Errors
/// Propagates any error Tauri's menu builders return.
pub fn build(app: &tauri::App) -> tauri::Result<Menu<Wry>> {
    let new_item = MenuItemBuilder::with_id(NEW_ID, "New")
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let open_item = MenuItemBuilder::with_id(OPEN_ID, "Open…")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let save_item = MenuItemBuilder::with_id(SAVE_ID, "Save")
        .accelerator("CmdOrCtrl+S")
        .build(app)?;
    let save_as_item = MenuItemBuilder::with_id(SAVE_AS_ID, "Save As…")
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = PredefinedMenuItem::quit(app, Some("Quit"))?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&new_item)
        .item(&open_item)
        .item(&save_item)
        .item(&save_as_item)
        .item(&separator)
        .item(&quit_item)
        .build()?;

    MenuBuilder::new(app).item(&file_menu).build()
}

/// Routes a menu event id to its handler in `main.rs`. Quit is a
/// [`PredefinedMenuItem`] and needs no entry here — Tauri handles it
/// directly.
pub fn dispatch(app: &AppHandle, id: &str) {
    match id {
        NEW_ID => crate::handle_new(app),
        OPEN_ID => crate::handle_open(app),
        SAVE_ID => crate::handle_save(app),
        SAVE_AS_ID => crate::handle_save_as(app),
        _ => {}
    }
}
