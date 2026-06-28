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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, Key};
use tauri::{AppHandle, Emitter, Manager};
use x11_clipboard::Clipboard;

use crate::danger;
use crate::trade;

const KEY_PRESS: i32 = 1;
const KEY_RELEASE: i32 = 0;

/// Guards against overlapping price checks. A re-triggered hotkey (e.g. mashing it
/// while a "rate limit — wait Ns" card is up) must not spawn a second concurrent
/// synth-and-GGG-search that could slip past the rate-limit lockout before it is
/// armed (ADR-0004 IP-ban safety); it also avoids a duplicate Ctrl+C into the game.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// RAII reset of [`IN_FLIGHT`] so every early return from `price_check` clears it.
struct InFlight;
impl Drop for InFlight {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

/// The uinput virtual keyboard, built once at startup and kept warm in Tauri
/// state so each price check reuses it.
pub struct Synth(pub Mutex<VirtualDevice>);

/// A warm X11 CLIPBOARD reader, kept in Tauri state. Opening a fresh `Clipboard`
/// (xfixes connection + helper thread) on every poll iteration races the selection
/// handshake and reads empty even when the game has set it — one reused connection
/// fixes that. Built via [`Reader::build`] and stored with `app.manage`.
pub struct Reader(pub Mutex<Clipboard>);

impl Reader {
    /// Open the X11 connection used to read the CLIPBOARD selection. Needs an X11
    /// display (XWayland is present on this KDE session).
    pub fn build() -> Result<Self, String> {
        Clipboard::new()
            .map(|c| Reader(Mutex::new(c)))
            .map_err(|e| e.to_string())
    }
}

/// Build the uinput device used to synthesize the in-game copy. Needs write
/// access to `/dev/uinput` (session ACL; no `input` group). Store the result in
/// Tauri state via `app.manage(Synth(Mutex::new(dev)))`.
pub fn build_synth() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    // C, plus every modifier `synth_copy` releases — a uinput device can only emit
    // keycodes it declared, so the pre-release would silently no-op without these.
    for k in [
        Key::KEY_C,
        Key::KEY_LEFTCTRL,
        Key::KEY_RIGHTCTRL,
        Key::KEY_LEFTALT,
        Key::KEY_RIGHTALT,
        Key::KEY_LEFTSHIFT,
        Key::KEY_RIGHTSHIFT,
        Key::KEY_LEFTMETA,
    ] {
        keys.insert(k);
    }
    VirtualDeviceBuilder::new()?
        .name("poe2-overlay virtual keyboard")
        .with_keys(&keys)?
        .build()
}

/// Run a price check: synthesize the copy, read the clipboard, parse + price the item,
/// and feed the overlay the two-phase contract (`price-check-loading` then
/// `price-check-result`; ADR-0004). Invoked by the single-instance handler on
/// `--price-check` (KDE's Ctrl+Alt+D shortcut).
///
/// Runs off the main thread (it sleeps) — the caller spawns it. Re-entrant triggers
/// while one check is in flight are dropped.
pub fn price_check(app: &AppHandle) {
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        eprintln!("[hotkey] price check already in flight; ignoring re-trigger");
        return;
    }
    let _in_flight = InFlight;

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
    let reader_state = app.try_state::<Reader>();
    let reader = reader_state.as_deref();
    let mut item = None;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(40));
        if let Some(text) = read_item(reader) {
            item = Some(text); // read_item only returns a real item (has "Item Class:")
            break;
        }
    }
    let Some(text) = item else {
        // Nothing was copied — no item under the cursor. Show the "No item" card rather
        // than returning silently, so a keypress always gives visible feedback (otherwise
        // the overlay looks dead when you trigger it off an item).
        eprintln!("[hotkey] clipboard still empty after ~800 ms — no item under the cursor?");
        let _ = app.emit("price-check-result", trade::PriceResult::invalid());
        show_overlay(app);
        return;
    };
    eprintln!("[hotkey] price check: {} chars copied", text.len());

    // Parse → price (T4). The parser is pure; pricing is async, so we block this
    // worker thread on Tauri's runtime (we are on a plain spawned std::thread, not a
    // runtime worker, so block_on is safe — ADR-0004).
    let Some(parsed) = trade::parse_item(&text) else {
        let _ = app.emit("price-check-result", trade::PriceResult::invalid());
        show_overlay(app);
        return;
    };

    // Waystone? Route to the danger-checker instead of pricing (T7, ADR-0005): a local,
    // instant, quota-free mod analysis emitted on `price-check-danger`. Waystones are not
    // priced — the danger verdict is what matters before running one.
    if danger::is_waystone(&parsed) {
        let report = danger::analyze(&parsed);
        eprintln!(
            "[hotkey] waystone danger: {:?} ({} flag(s))",
            report.level,
            report.flags.len()
        );
        let _ = app.emit("price-check-danger", report);
        show_overlay(app);
        return;
    }

    // Two-phase contract (ADR-0004): `price-check-loading` shows the card immediately
    // while the trade2 round-trip runs; `price-check-result` carries the listings.
    let name = trade::display_name(&parsed);
    let _ = app.emit("price-check-loading", &name);
    show_overlay(app);

    let Some(pricing) = app.try_state::<trade::Pricing>() else {
        eprintln!("[hotkey] pricing state unavailable");
        return;
    };
    let result = tauri::async_runtime::block_on(pricing.price(&parsed));
    eprintln!(
        "[hotkey] {name} → {:?} ({} listings)",
        result.status,
        result.listings.len()
    );
    let _ = app.emit("price-check-result", result);
}

/// Map the overlay surface only when hidden. Calling `show()` on an already-mapped
/// gtk-layer-shell window commits a *second* surface and the previous frame lingers
/// as a stacked ghost; when already visible the emitted event updates the card in place.
fn show_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if !w.is_visible().unwrap_or(false) {
            let _ = w.show();
        }
    }
}

/// Synthesize a clean Ctrl+C — PoE2's copy-item-under-cursor.
///
/// KWin's global-shortcut grab on `Ctrl+Alt+D` stops the *game* from seeing the chord,
/// but does NOT clear the seat's modifier state: at synth time the physical LEFTCTRL +
/// LEFTALT are still held, so a bare `C` is delivered as `Ctrl+Alt+C` (or with a stray
/// modifier) and the wrong text — or nothing — lands on the clipboard. So first release
/// every modifier (a per-keycode key-up clears it in the seat even while the physical key
/// is held), then drive a well-spaced Ctrl-down → C → Ctrl-up so a frame-cadence game
/// samples Ctrl as held across the C edge. `emit()` appends a SYN_REPORT per call, so the
/// gaps between calls are what give the game time to register each transition.
fn synth_copy(synth: &Mutex<VirtualDevice>) -> std::io::Result<()> {
    let mut dev = synth.lock().unwrap_or_else(|e| e.into_inner());
    let down = |k: Key| InputEvent::new(EventType::KEY, k.code(), KEY_PRESS);
    let up = |k: Key| InputEvent::new(EventType::KEY, k.code(), KEY_RELEASE);

    // 1) Clear any held modifier so the seat baseline is clean.
    for m in [
        Key::KEY_LEFTCTRL,
        Key::KEY_RIGHTCTRL,
        Key::KEY_LEFTALT,
        Key::KEY_RIGHTALT,
        Key::KEY_LEFTSHIFT,
        Key::KEY_RIGHTSHIFT,
        Key::KEY_LEFTMETA,
    ] {
        dev.emit(&[up(m)])?;
    }
    thread::sleep(Duration::from_millis(25)); // let the compositor settle the cleared mods

    // 2) Clean, spaced Ctrl+C.
    dev.emit(&[down(Key::KEY_LEFTCTRL)])?;
    thread::sleep(Duration::from_millis(30)); // Ctrl latched before C
    dev.emit(&[down(Key::KEY_C)])?;
    thread::sleep(Duration::from_millis(40)); // hold across several frames
    dev.emit(&[up(Key::KEY_C)])?;
    thread::sleep(Duration::from_millis(15));
    dev.emit(&[up(Key::KEY_LEFTCTRL)])?;
    Ok(())
}

/// Read the freshly-copied PoE2 item text. The Proton/XWayland game writes the X11
/// CLIPBOARD selection, but on some compositors KDE's Wayland mirror is what holds it —
/// so try the warm X11 reader first (UTF8_STRING then STRING), then `wl-paste` and
/// `xclip`, and accept the first source that holds a real item. Returns only text
/// containing the `Item Class:` header so a stale non-item clipboard is ignored.
fn read_item(reader: Option<&Reader>) -> Option<String> {
    if let Some(r) = reader {
        let clip = r.0.lock().unwrap_or_else(|e| e.into_inner());
        for target in [clip.getter.atoms.utf8_string, clip.getter.atoms.string] {
            if let Ok(bytes) = clip.load(
                clip.getter.atoms.clipboard,
                target,
                clip.getter.atoms.property,
                Duration::from_millis(80),
            ) {
                if let Ok(s) = String::from_utf8(bytes) {
                    if s.contains("Item Class:") {
                        return Some(s);
                    }
                }
            }
        }
    }
    // Fallbacks for whichever selection actually holds the copy on this compositor.
    if let Some(s) = read_cmd("wl-paste", &[]) {
        if s.contains("Item Class:") {
            return Some(s);
        }
    }
    if let Some(s) = read_cmd("xclip", &["-selection", "clipboard", "-o"]) {
        if s.contains("Item Class:") {
            return Some(s);
        }
    }
    None
}

/// Run a clipboard-reader binary; `None` if it is missing or exits non-zero.
fn read_cmd(bin: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(bin).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
