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

use tauri::{Manager, WebviewWindow};

/// Hide the overlay surface. The probe panel's close control (and Esc) invoke this;
/// T3 replaces show-on-launch with an evdev hotkey that toggles visibility.
#[tauri::command]
fn hide_overlay(window: WebviewWindow) {
    let _ = window.hide();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![hide_overlay])
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            // Promote to a layer-shell OVERLAY surface while still hidden, then
            // reveal it. T3 will gate showing behind the global hotkey; for now it
            // shows on launch so the surface can be verified over the game.
            overlay::init_layer_shell(&window)?;
            window.show()?;
            Ok(())
        })
        // TODO(T3): start the global hotkey listener via `hotkey::start_listener`.
        .run(tauri::generate_context!())
        .expect("error while running PoE2 Overlay");
}
