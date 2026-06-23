//! PoE2 Overlay — a Wayland-native Path of Exile 2 trade overlay for KDE Plasma.
//!
//! Architecture (see `docs/adr/0001-*.md`): the overlay draws over a fullscreen
//! Proton game by promoting the Tauri GTK window to a `wlr-layer-shell` surface
//! on the OVERLAY layer (`overlay`), reads the hovered item by synthesizing
//! Ctrl+C into the game and reading the clipboard (`hotkey` + clipboard), and
//! prices it against the GGG trade2 API + poe.ninja (`trade`). The pieces are
//! stubbed here and wired per the active plan (T2-T4).

mod hotkey;
mod overlay;
mod trade;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // TODO(T2): on setup, promote the main window to a layer-shell OVERLAY
        // surface via `overlay::init_layer_shell`, and start the global hotkey
        // listener via `hotkey::start_listener`.
        .run(tauri::generate_context!())
        .expect("error while running PoE2 Overlay");
}
