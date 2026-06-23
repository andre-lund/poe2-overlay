//! Global hotkey + item-copy input path.
//!
//! On Wayland an app cannot grab global keys the X11 way, so we read keyboards
//! directly via `evdev` (works regardless of compositor focus) and synthesize
//! the in-game copy (Ctrl+C, or PoE2's Ctrl+Alt+C advanced-copy) via a kernel
//! `uinput` virtual device. The copied item is then read from the clipboard —
//! preferring the X11 selection (`xclip`) over `wl-paste`, which returns KWin's
//! stale clipboard for XWayland/Proton clients (see ADR-0001, docs/research).

/// Start the global hotkey listener (evdev) that triggers a price check.
pub fn start_listener() {
    // TODO(T3): requires the `evdev` crate + user in the `input` group (or the
    // /dev/uinput session ACL). Reference: ExileWatch lib.rs L971-1094.
    unimplemented!("evdev hotkey + uinput Ctrl+C — plan T3")
}
