//! PoE2 Overlay — a Wayland-native Path of Exile 2 trade overlay for KDE Plasma.
//!
//! Architecture (see `docs/adr/0001-*.md`): the overlay draws over a fullscreen
//! Proton game by promoting the Tauri GTK window to a `wlr-layer-shell` surface
//! on the OVERLAY layer (`overlay`), reads the hovered item by synthesizing
//! Ctrl+C into the game and reading the clipboard (`hotkey` + clipboard), and
//! prices it against the GGG trade2 API + poe.ninja (`trade`). The pieces are
//! stubbed here and wired per the active plan (T2-T4).

mod danger;
mod hotkey;
mod overlay;
mod trade;

use tauri::{Manager, State, WebviewWindow};

/// Hide the overlay surface. The card's ✕ control (and Esc) invoke this; the price
/// check shows it again on the next Ctrl+Alt+D.
#[tauri::command]
fn hide_overlay(window: WebviewWindow) {
    let _ = window.hide();
}

/// Re-price the last-checked item with the overlay's edited filters + selected league
/// (T5 toggles + league selector). Always `Ok` — pricing failures arrive as a
/// `PriceResult` with an error/rate-limited status, not a command error.
#[tauri::command]
async fn requery(
    pricing: State<'_, trade::Pricing>,
    league: String,
    parsed_stats: Vec<trade::ParsedStat>,
    base_properties: Vec<trade::BaseProp>,
) -> Result<trade::PriceResult, ()> {
    Ok(pricing.requery(league, parsed_stats, base_properties).await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the FIRST plugin. KDE's Ctrl+Alt+D shortcut launches
        // `poe2-overlay --price-check`; this forwards that to the running
        // instance and exits the second one (ADR-0002).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|a| a == "--price-check") {
                let app = app.clone();
                // Off the event-loop thread — price_check sleeps.
                std::thread::spawn(move || hotkey::price_check(&app));
            } else if argv.iter().any(|a| a == "--hide") {
                // Compositor-level dismiss (ADR-0003): KDE forwards `--hide` here.
                // KWin owns the shortcut, so it always reaches us even if the
                // OVERLAY surface were grabbing all pointer/keyboard input — the
                // guaranteed escape from any input-trap, plus the normal close key.
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![hide_overlay, requery])
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            // Promote to a layer-shell OVERLAY surface while still hidden. The
            // window stays hidden until a price check shows it; ✕/Esc hides it.
            overlay::init_layer_shell(&window)?;
            // Build the uinput synth device once and keep it warm in state.
            match hotkey::build_synth() {
                Ok(dev) => {
                    app.manage(hotkey::Synth(std::sync::Mutex::new(dev)));
                }
                Err(e) => eprintln!("[hotkey] cannot open /dev/uinput ({e}); item copy disabled"),
            }
            // Warm pricing client + caches kept in state across checks (T4, ADR-0004).
            app.manage(trade::Pricing::new());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PoE2 Overlay");
}
