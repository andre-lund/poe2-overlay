//! Wayland layer-shell overlay surface — the part that draws over the game.
//!
//! Promotes the Tauri GTK window to a `wlr-layer-shell` surface on the OVERLAY
//! layer so KWin composites it above the fullscreen Proton game. This is the
//! mechanism that works on native KDE Wayland, where the X11 overlays used by
//! Exiled-Exchange-2 / awakened-poe-trade fail (see ADR-0001 and
//! docs/research/RESEARCH.md).

use tauri::WebviewWindow;

#[cfg(target_os = "linux")]
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

/// Promote `window` to a wlr-layer-shell OVERLAY surface, anchored to the
/// top-right corner and sized to the window (NOT stretched to the whole output).
///
/// Must run while the window is still hidden (`visible: false` in
/// tauri.conf.json): gtk-layer-shell requires promotion *before* the GTK window
/// is mapped. Reference: PathofTrading / ExileWatch (technique only — ADR-0001).
///
/// **Why full-output, not corner-sized:** anchoring to all four edges forces the
/// surface to the whole output, which renders reliably. A corner surface (two
/// anchors) takes its size from the GTK window's child — the WebKitGTK webview,
/// whose minimum size request is ~0 — so the surface collapses and nothing draws.
/// The overlay is therefore modal while shown (it covers the screen) and dismissed
/// with its own close control + Esc (see `hide_overlay`), the same full-screen,
/// focusable, show-on-demand model the proven PathofTrading reference uses. T3 hides
/// it by default and shows it on the hotkey; per-region click-through (so the game
/// stays live behind a visible panel) is a T5 problem — tao's
/// `set_ignore_cursor_events` sets the input shape on the toplevel GDK window, which
/// the WebKitGTK child surface ignores, so it does not achieve click-through here.
#[cfg(target_os = "linux")]
pub fn init_layer_shell(window: &WebviewWindow) -> tauri::Result<()> {
    let gtk_window = window.gtk_window()?;

    gtk_window.init_layer_shell();
    gtk_window.set_layer(Layer::Overlay);
    gtk_window.set_namespace("poe2-overlay");

    // Anchor all four edges → the surface fills the output (a full-screen canvas the
    // card is positioned within via CSS). Required for the surface to get a non-zero
    // size; see the doc comment above.
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        gtk_window.set_anchor(edge, true);
    }
    // Draw over panels; reserve no screen space.
    gtk_window.set_exclusive_zone(-1);
    // Keyboard reaches the surface once the user clicks it (OnDemand) — enough for
    // Esc-to-dismiss after a click; the ✕ button works on click regardless. The game
    // keeps keyboard focus until then. (Exclusive was rejected — it drops the game
    // out of fullscreen.)
    gtk_window.set_keyboard_mode(KeyboardMode::OnDemand);

    Ok(())
}

/// No-op off Linux — the overlay only targets KDE Plasma Wayland (ADR-0001), but
/// keep the symbol so non-Linux builds compile.
#[cfg(not(target_os = "linux"))]
pub fn init_layer_shell(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}
