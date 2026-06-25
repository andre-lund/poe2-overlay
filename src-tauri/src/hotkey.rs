//! Item-copy input path (trigger side lives in KDE — see ADR-0002).
//!
//! The price-check trigger is a KWin global shortcut (`Ctrl+Alt+D`) that launches
//! `poe2-overlay --price-check`; `tauri-plugin-single-instance` forwards that to
//! the running app, which calls [`price_check`]. KWin consumes the chord, so the
//! game never sees the keys (no character movement) and we never read
//! `/dev/input`. Here we only: synthesize the in-game copy via a `uinput` virtual
//! device (Ctrl+C) and read the item text from the X11 CLIPBOARD selection —
//! `wl-paste` returns KWin's stale clipboard for XWayland/Proton clients.
//!
//! Permissions: writing `/dev/uinput` needs the session ACL (already granted on
//! this machine); no `input` group.

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, Key};
use tauri::{AppHandle, Emitter, Manager};
use x11_clipboard::Clipboard;

const KEY_PRESS: i32 = 1;
const KEY_RELEASE: i32 = 0;

/// The uinput virtual keyboard, built once at startup and kept warm in Tauri
/// state so each price check reuses it.
pub struct Synth(pub Mutex<VirtualDevice>);

/// Build the uinput device used to synthesize the in-game copy. Needs write
/// access to `/dev/uinput` (session ACL; no `input` group). Store the result in
/// Tauri state via `app.manage(Synth(Mutex::new(dev)))`.
pub fn build_synth() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for k in [Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_C] {
        keys.insert(k);
    }
    VirtualDeviceBuilder::new()?
        .name("poe2-overlay virtual keyboard")
        .with_keys(&keys)?
        .build()
}

/// Run a price check: synthesize the copy, read the clipboard, hand the item text
/// to the overlay (event `price-check-item` + show). Invoked by the
/// single-instance handler on `--price-check` (KDE's Ctrl+Alt+D shortcut).
///
/// Runs off the main thread (it sleeps) — the caller spawns it.
pub fn price_check(app: &AppHandle) {
    let Some(synth) = app.try_state::<Synth>() else {
        eprintln!("[hotkey] synth device unavailable (is /dev/uinput writable?)");
        return;
    };
    if let Err(e) = synth_copy(&synth.0) {
        eprintln!("[hotkey] synth copy failed: {e}");
        return;
    }
    // The game's copy reaches the X11 CLIPBOARD only after KWin's XWayland clipboard
    // sync, which is racy: a single read often catches a transient-empty state mid-sync,
    // so the item appears only on a *later* read (PathofTrading retries for exactly this
    // reason). Poll until the clipboard holds a non-empty item, or give up (~800 ms).
    let mut item = None;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(40));
        if let Some(text) = read_x11_clipboard() {
            if !text.trim().is_empty() {
                item = Some(text);
                break;
            }
        }
    }
    match item {
        Some(text) => {
            eprintln!("[hotkey] price check: {} chars copied", text.len());
            let _ = app.emit("price-check-item", text);
            if let Some(w) = app.get_webview_window("main") {
                // Only map the surface when it is actually hidden. Calling show() on an
                // already-mapped gtk-layer-shell window commits a *second* surface and the
                // previous frame lingers as a stacked ghost; when the overlay is already
                // visible the emitted event updates the card content in place.
                if !w.is_visible().unwrap_or(false) {
                    let _ = w.show();
                }
            }
        }
        None => eprintln!("[hotkey] clipboard still empty after ~800 ms — no item under the cursor?"),
    }
}

/// Synthesize Ctrl+C — PoE2's copy-item-under-cursor (basic). KDE consumes the
/// Ctrl+Alt+D chord, so the game's modifier state is clean here; for advanced
/// item text (affix tiers/ranges) add `Key::KEY_LEFTALT` to the pairs (Ctrl+Alt+C).
fn synth_copy(synth: &Mutex<VirtualDevice>) -> std::io::Result<()> {
    let mut dev = synth.lock().unwrap();
    let down = |k: Key| InputEvent::new(EventType::KEY, k.code(), KEY_PRESS);
    let up = |k: Key| InputEvent::new(EventType::KEY, k.code(), KEY_RELEASE);

    dev.emit(&[down(Key::KEY_LEFTCTRL)])?;
    dev.emit(&[down(Key::KEY_C)])?;
    thread::sleep(Duration::from_millis(12)); // let the game register the key-down
    dev.emit(&[up(Key::KEY_C)])?;
    dev.emit(&[up(Key::KEY_LEFTCTRL)])?;
    Ok(())
}

/// Read the X11 CLIPBOARD selection (XWayland/Proton writes the item text there).
fn read_x11_clipboard() -> Option<String> {
    let clip = Clipboard::new().ok()?;
    let bytes = clip
        .load(
            clip.getter.atoms.clipboard,
            clip.getter.atoms.utf8_string,
            clip.getter.atoms.property,
            Duration::from_millis(200),
        )
        .ok()?;
    String::from_utf8(bytes).ok()
}
