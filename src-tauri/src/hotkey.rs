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
    let Some(text) = item else {
        eprintln!("[hotkey] clipboard still empty after ~800 ms — no item under the cursor?");
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

/// Synthesize Ctrl+C — PoE2's copy-item-under-cursor (basic). KDE consumes the
/// Ctrl+Alt+D chord, so the game's modifier state is clean here; for advanced
/// item text (affix tiers/ranges) add `Key::KEY_LEFTALT` to the pairs (Ctrl+Alt+C).
fn synth_copy(synth: &Mutex<VirtualDevice>) -> std::io::Result<()> {
    let mut dev = synth.lock().unwrap_or_else(|e| e.into_inner());
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
