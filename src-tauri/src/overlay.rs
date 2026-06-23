//! Wayland layer-shell overlay surface — the part that draws over the game.
//!
//! Promotes the Tauri GTK window to a `wlr-layer-shell` surface on the OVERLAY
//! layer so KWin composites it above the fullscreen Proton game. This is the
//! mechanism that works on native KDE Wayland, where the X11 overlays used by
//! Exiled-Exchange-2 / awakened-poe-trade fail (see ADR-0001 and
//! docs/research/RESEARCH.md).

/// Promote the given window to a layer-shell overlay surface: `Layer::Overlay`,
/// edge anchors, an empty input region for click-through, and on-demand keyboard
/// interactivity so the game keeps focus until the overlay is shown.
pub fn init_layer_shell() {
    // TODO(T2): requires the `gtk-layer-shell` crate + system lib
    // (`pacman -S gtk-layer-shell gtk3`). Reference: ExileWatch lib.rs L895-937.
    unimplemented!("layer-shell init — plan T2")
}
