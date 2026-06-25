//! X11 CLIPBOARD write for the regex cheat-sheet (plan T8, ADR-0006).
//!
//! The overlay only *read* the X11 selection before (item copy, `hotkey`); the
//! cheat-sheet needs to *write* it so the user can paste a pattern into the game's
//! Ctrl-F box. We own the X11 CLIPBOARD selection directly (not the Wayland clipboard)
//! because the Proton/XWayland game reads X11 — the write-side mirror of the read in
//! `hotkey::read_x11_clipboard`. The owning [`Clipboard`] lives in Tauri state for the
//! process lifetime so its background thread keeps serving paste requests (the warm
//! instance persists the whole session); `store` itself is non-blocking.

use std::sync::Mutex;

use x11_clipboard::Clipboard;

/// The persistent X11 clipboard owner, kept in Tauri state via `app.manage(Clip::build()?)`.
pub struct Clip(Mutex<Clipboard>);

impl Clip {
    /// Open the X11 connection that will own the CLIPBOARD selection. Needs an X11
    /// display (XWayland is present on this KDE session).
    pub fn build() -> Result<Self, String> {
        Clipboard::new()
            .map(|c| Clip(Mutex::new(c)))
            .map_err(|e| e.to_string())
    }

    /// Put `text` on the X11 CLIPBOARD selection (UTF-8). The game's Ctrl+V reads it.
    /// Poison-recovering so a stray panic can't brick the copy path.
    pub fn copy(&self, text: &str) -> Result<(), String> {
        let clip = self.0.lock().unwrap_or_else(|e| e.into_inner());
        clip.store(
            clip.getter.atoms.clipboard,
            clip.getter.atoms.utf8_string,
            text.as_bytes().to_vec(),
        )
        .map_err(|e| e.to_string())
    }
}
