//! Wayland layer-shell overlay surface — the part that draws over the game.
//!
//! Promotes the Tauri GTK window to a `wlr-layer-shell` surface on the OVERLAY
//! layer so KWin composites it above the fullscreen Proton game. This is the
//! mechanism that works on native KDE Wayland, where the X11 overlays used by
//! Exiled-Exchange-2 / awakened-poe-trade fail (see ADR-0001 and
//! docs/research/RESEARCH.md).

use tauri::WebviewWindow;

#[cfg(target_os = "linux")]
use gtk::prelude::{GtkWindowExt, WidgetExt};
#[cfg(target_os = "linux")]
use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};

/// Fixed size of the panel surface, in logical px. Large enough for the T5 price
/// card (league selector + filter toggles + listings, scrolling internally); small
/// enough that its bounded input region only covers a central patch.
#[cfg(target_os = "linux")]
const OVERLAY_W: i32 = 470;
#[cfg(target_os = "linux")]
const OVERLAY_H: i32 = 640;

/// Promote `window` to a wlr-layer-shell OVERLAY surface, **centered** on the
/// output at a fixed size — NOT stretched to the whole output.
///
/// Must run while the window is still hidden (`visible: false` in
/// tauri.conf.json): gtk-layer-shell requires promotion *before* the GTK window
/// is mapped. Reference: PathofTrading / ExileWatch (technique only — ADR-0001).
///
/// **Why a sized sub-output surface, not full-output (ADR-0003):** a four-edge
/// (full-output) surface covers the entire screen, and a `wl_surface`'s input
/// region defaults to the whole surface — CSS `pointer-events: none` does *not*
/// shrink it — so the surface swallows every click on the desktop and traps all
/// input with no escape but its own on-screen controls. A surface anchored to no
/// opposite-edge pair is not stretched; it stays a finite rectangle (centered when
/// unanchored), so clicks outside it reach the game. Forcing the size needs
/// gtk-layer-shell's two-call "Forcing Window Size" idiom: tao pre-sizes the GTK
/// window to the `tauri.conf.json` dimensions (1140×600) via `resize`, and on a
/// resizable toplevel `set_size_request` only raises the *minimum* — so it alone
/// would map ~1140 wide. The following `resize(1, 1)` is clamped back up to the
/// `set_size_request` minimum and clears tao's sticky size, committing the true
/// `OVERLAY_W × OVERLAY_H` surface. Worst case (size handling ever failing) the
/// surface is ~1×1 — invisible, trapping nothing — never full-screen. Per-region
/// click-through *within* the panel (so the game stays live behind it) is the T5 seam.
///
/// **Keyboard mode `OnDemand` (ADR-0007):** the surface takes keyboard focus only
/// after a click on the panel — so the typed filters (stat min/max, sheet name
/// filter) work — and the game keeps the keyboard at all other times. Dismissal
/// stays focus-independent via the ✕ button (a pointer click) and the `Ctrl+Alt+X`
/// hide shortcut (KWin); in-webview Esc also works while the panel holds focus.
/// (`Exclusive` remains rejected — it drops the game out of fullscreen.)
#[cfg(target_os = "linux")]
pub fn init_layer_shell(window: &WebviewWindow) -> tauri::Result<()> {
    let gtk_window = window.gtk_window()?;

    gtk_window.init_layer_shell();
    gtk_window.set_layer(Layer::Overlay);
    gtk_window.set_namespace("poe2-overlay");

    // Anchor no edges → gtk-layer-shell centers the surface on the output. With no
    // opposite-edge pair anchored it is never stretched, so it stays a finite
    // centered rectangle.
    // Force a concrete size (gtk-layer-shell "Forcing Window Size" idiom). tao has
    // already pinned the window to the tauri.conf.json size (1140×600); set_size_request
    // only raises the minimum, so the resize(1,1) — clamped up to that minimum — is what
    // clears tao's sticky size and commits the OVERLAY_W×OVERLAY_H surface. Both calls
    // are required; see the doc comment.
    gtk_window.set_size_request(OVERLAY_W, OVERLAY_H);
    gtk_window.resize(1, 1);

    // Draw over panels; reserve no screen space.
    gtk_window.set_exclusive_zone(-1);
    // Focus only on click (ADR-0007) — the game keeps the keyboard until the user
    // clicks a typed filter; dismissal stays the ✕ click + Ctrl+Alt+X regardless.
    gtk_window.set_keyboard_mode(KeyboardMode::OnDemand);

    Ok(())
}

/// No-op off Linux — the overlay only targets KDE Plasma Wayland (ADR-0001), but
/// keep the symbol so non-Linux builds compile.
#[cfg(not(target_os = "linux"))]
pub fn init_layer_shell(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}
