//! Item-copy input path (trigger side lives in KDE — see ADR-0002).
//!
//! The price-check trigger is a KWin global shortcut (`Ctrl+Alt+D`) that launches
//! `poe2-overlay --price-check`; `tauri-plugin-single-instance` forwards that to
//! the running app, which calls [`price_check`]. KWin consumes the chord, so the
//! game never sees the keys (no character movement) and we never read
//! `/dev/input`.
//!
//! Layout (plan 0002 T1): [`price_check`] — hide card, synthesize the copy, poll
//! the clipboard for a fresh item, parse, danger-check or price, emit — is shared
//! across platforms. The three primitives it needs (the [`Synth`] device,
//! `synth_copy`, `read_clipboard`) live in the per-OS `platform` module: Linux =
//! uinput/ydotool + `wl-paste` (permissions: `/dev/uinput` session ACL, no `input`
//! group); Windows = stubs until plan 0002 T3 (SendInput + arboard).

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::danger;
use crate::trade;

pub use platform::{build_synth, Synth};
use platform::{read_clipboard, synth_copy};

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

    // Snapshot the clipboard BEFORE the copy. Clearing it doesn't work here (wl-copy
    // --clear drops the Wayland owner and KWin just re-presents the stale X11 selection),
    // so instead we wait for the content to actually CHANGE to the new item — reading too
    // early otherwise returns the previous item (the off-by-one bug).
    let before = read_clipboard();

    // Hide any card still on screen before copying: the centred overlay captures the
    // pointer, so a visible card sitting over the cursor stops the game from registering
    // the hovered item (then Ctrl+C copies nothing). The new result re-shows it.
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }

    let Some(synth) = app.try_state::<Synth>() else {
        eprintln!("[hotkey] synth device unavailable (is /dev/uinput writable?)");
        return;
    };
    if let Err(e) = synth_copy(&synth) {
        eprintln!("[hotkey] synth copy failed: {e}");
        return;
    }
    // The game's copy reaches the clipboard only after KWin's XWayland sync, which lags a
    // few hundred ms. Poll until the clipboard CHANGES to a fresh item (vs the snapshot),
    // or give up (~1.5 s → re-price an unchanged item-shaped clipboard, else "No item").
    // Waiting for the change is what kills the off-by-one
    // (reading too early returns the previous item). But the FIRST synth of a session often
    // does not land — ydotool's daemon is cold, or KWin's shortcut grab still holds the
    // physical keys as we synthesize — which is the "first press shows nothing" symptom. So
    // re-fire the copy once, partway through, if nothing has changed yet. Re-firing is safe:
    // we still only accept a clipboard that actually changed, so a re-synth can never surface
    // a stale item.
    let mut item = None;
    for i in 0..30 {
        thread::sleep(Duration::from_millis(50));
        let now = read_clipboard();
        if now != before && now.contains("Item Class:") {
            item = Some(now);
            break;
        }
        // ~0.5 s in with no fresh item: assume the first synth was dropped and try again.
        if i == 10 {
            let _ = synth_copy(&synth);
        }
    }
    let text = match item {
        Some(t) => t,
        // The clipboard never changed — but it already holds an item. That is the
        // re-check-the-same-item case (the game copies identical text, so "changed"
        // can never fire): price what's there instead of showing "No item". The
        // trade-off: pressing the key over *nothing* while an old item text sits on
        // the clipboard re-prices that old item — the card names the item prominently,
        // so a stale re-price is self-explanatory where a false "No item" was a dead end.
        None if before.contains("Item Class:") => {
            eprintln!("[hotkey] clipboard unchanged but holds an item — re-pricing it (same-item re-check)");
            before.clone()
        }
        None => {
            // No fresh item and no item-shaped clipboard — nothing under the cursor.
            // Show the "No item" card so a keypress always gives visible feedback
            // rather than the overlay looking dead.
            eprintln!("[hotkey] no item on the clipboard after ~1.5 s — no item under the cursor?");
            let _ = app.emit("price-check-result", trade::PriceResult::invalid());
            show_overlay(app);
            return;
        }
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

/// Linux input/clipboard primitives: uinput/ydotool Ctrl+C synth + `wl-paste` read.
#[cfg(target_os = "linux")]
mod platform {
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

    use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
    use evdev::{AttributeSet, EventType, InputEvent, Key};

    const KEY_PRESS: i32 = 1;
    const KEY_RELEASE: i32 = 0;

    /// The uinput virtual keyboard, built once at startup and kept warm in Tauri
    /// state so each price check reuses it.
    pub struct Synth(Mutex<VirtualDevice>);

    /// Build the uinput device used to synthesize the in-game copy. Needs write
    /// access to `/dev/uinput` (session ACL; no `input` group). Store the result in
    /// Tauri state via `app.manage(...)`.
    pub fn build_synth() -> std::io::Result<Synth> {
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
        let dev = VirtualDeviceBuilder::new()?
            .name("poe2-overlay virtual keyboard")
            .with_keys(&keys)?
            .build()?;
        Ok(Synth(Mutex::new(dev)))
    }

    /// Synthesize the in-game copy (Ctrl+C). Prefers **ydotool**: on this KDE Wayland +
    /// XWayland/Proton setup our own evdev uinput device's key events don't reach PoE2 (a
    /// manual Ctrl+C copies fine, ours doesn't), but ydotool's daemon device — the mechanism
    /// the PathofTrading reference proved on this exact machine — does. Falls back to the
    /// evdev device when ydotool's daemon socket isn't up.
    pub fn synth_copy(synth: &Synth) -> std::io::Result<()> {
        if synth_copy_ydotool() {
            return Ok(());
        }
        synth_copy_evdev(&synth.0)
    }

    /// Drive Ctrl+C through ydotool's daemon (keycode 29 = LEFTCTRL, 46 = C; ydotool spaces
    /// the events itself). Returns `true` on success; `false` (a no-op) when the daemon socket
    /// is absent or ydotool is missing, so the caller falls back to the evdev device.
    fn synth_copy_ydotool() -> bool {
        let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
            return false;
        };
        let socket = std::path::Path::new(&runtime).join(".ydotool_socket");
        if !socket.exists() {
            return false;
        }
        std::process::Command::new("ydotool")
            .env("YDOTOOL_SOCKET", &socket)
            .args(["key", "29:1", "46:1", "46:0", "29:0"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Fallback synth via our own evdev uinput device.
    ///
    /// KWin's global-shortcut grab on `Ctrl+Alt+D` stops the *game* from seeing the chord,
    /// but does NOT clear the seat's modifier state: at synth time the physical LEFTCTRL +
    /// LEFTALT are still held, so a bare `C` is delivered as `Ctrl+Alt+C` (or with a stray
    /// modifier) and the wrong text — or nothing — lands on the clipboard. So first release
    /// every modifier (a per-keycode key-up clears it in the seat even while the physical key
    /// is held), then drive a well-spaced Ctrl-down → C → Ctrl-up so a frame-cadence game
    /// samples Ctrl as held across the C edge. `emit()` appends a SYN_REPORT per call, so the
    /// gaps between calls are what give the game time to register each transition.
    fn synth_copy_evdev(synth: &Mutex<VirtualDevice>) -> std::io::Result<()> {
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

    /// Current clipboard text (Wayland, via `wl-paste`); empty string if unreadable. KWin
    /// syncs the Proton/XWayland game's copy to the Wayland clipboard, and `wl-paste` is what
    /// the working reference read on this machine. Used to snapshot the clipboard before a
    /// copy and to poll for the change afterwards.
    pub fn read_clipboard() -> String {
        let Ok(out) = std::process::Command::new("wl-paste").output() else {
            return String::new();
        };
        if out.status.success() {
            String::from_utf8_lossy(&out.stdout).into_owned()
        } else {
            String::new()
        }
    }
}

/// Non-Linux stubs (plan 0002 T1). The real Windows primitives — `SendInput` for the
/// Ctrl+C synth, `arboard` for the clipboard — land in plan 0002 T3; until then
/// `build_synth` errors so startup logs one clear "item copy disabled" line and
/// `price_check` exits early on the missing state.
#[cfg(not(target_os = "linux"))]
mod platform {
    pub struct Synth;

    pub fn build_synth() -> std::io::Result<Synth> {
        Err(std::io::Error::other(
            "input synth not implemented on this platform yet (plan 0002 T3)",
        ))
    }

    pub fn synth_copy(_synth: &Synth) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "input synth not implemented on this platform yet (plan 0002 T3)",
        ))
    }

    pub fn read_clipboard() -> String {
        String::new()
    }
}
