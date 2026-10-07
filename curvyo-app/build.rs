//! Tauri's build-time codegen (icon embedding, `tauri.conf.json` parsing
//! for `tauri::generate_context!()`).

fn main() {
    tauri_build::build();
}
