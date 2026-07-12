//! Clipboard write for the regex cheat-sheet (plan T8, ADR-0006).
//!
//! The overlay only *read* the clipboard before (item copy, `hotkey`); the
//! cheat-sheet needs to *write* it so the user can paste a pattern into the game's
//! Ctrl-F box. Platform seam (plan 0002 T1): on Linux we own the X11 CLIPBOARD
//! selection directly (not the Wayland clipboard) because the Proton/XWayland game
//! reads X11 — the write-side mirror of the read in `hotkey`; the owning
//! [`Clip`] lives in Tauri state for the process lifetime so its background thread
//! keeps serving paste requests, and `copy` itself is non-blocking. On Windows the
//! write is stubbed until plan 0002 T3 (arboard).

pub use platform::Clip;

#[cfg(target_os = "linux")]
mod platform {
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
}

#[cfg(not(target_os = "linux"))]
mod platform {
    /// Non-Linux stub (plan 0002 T1): `build` succeeds so the Tauri state + command
    /// wiring stays identical across platforms; `copy` errors until the arboard
    /// implementation lands (plan 0002 T3).
    pub struct Clip;

    impl Clip {
        pub fn build() -> Result<Self, String> {
            Ok(Clip)
        }

        pub fn copy(&self, _text: &str) -> Result<(), String> {
            Err("clipboard write not implemented on this platform yet (plan 0002 T3)".into())
        }
    }
}
