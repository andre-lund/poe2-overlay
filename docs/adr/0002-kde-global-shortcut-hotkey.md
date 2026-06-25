---
status: accepted
---

# 0002 — Capture the price-check hotkey via a KDE global shortcut (single-instance forwarding), not evdev

**Implemented by:** [docs/plans/active/0001-mvp-price-check-overlay.md](../plans/active/0001-mvp-price-check-overlay.md) (T3)
**Supersedes:** ADR-0001 (the input-capture mechanism only — its clean-build, Rust/Tauri stack, layer-shell overlay, and pricing decisions still stand)

The price-check trigger is a **KDE global shortcut** (default `Ctrl+Alt+D`) that runs
`poe2-overlay --price-check`; `tauri-plugin-single-instance` forwards that invocation to
the already-running app, which performs the check. Item copy is still a `uinput`-synthesized
Ctrl+C and the clipboard is still read from the X11 selection — only the *trigger* mechanism
changes from raw `evdev`.

## Context

ADR-0001 chose raw `evdev` to read a global hotkey "compositor-focus-independent." In testing
that broke: `evdev` can *read* keys but cannot *consume* them, so a chord on a game-bound key
(PoE2 binds `D`) leaks to the game and moves the character — confirmed live (`Alt+D` moved the
character). It also required adding the user to the `input` group, a keylogger-capable
privilege. KWin global shortcuts instead intercept the chord *before* any client sees it — the
exact mechanism PathofTrading (validated on this machine, bound to `Ctrl+Alt+D`) uses. We are
KDE-only (ADR-0001), so coupling to KWin's shortcut system is acceptable.

## Decision

- **Trigger:** a KWin global shortcut owns `Ctrl+Alt+D` and launches `poe2-overlay --price-check`.
  KWin consumes the chord, so the game never receives `D` (no movement) and we never read
  `/dev/input`.
- **Forwarding:** `tauri-plugin-single-instance` makes the second invocation hand its args to
  the running instance (kept warm for pricing per ADR-0001) and exit; the running app runs the
  check on `--price-check`.
- **Copy + clipboard unchanged:** `uinput` synthesizes Ctrl+C (the `/dev/uinput` session ACL is
  sufficient — no `input` group), then the X11 CLIPBOARD selection is read.
- We take over PathofTrading's `Ctrl+Alt+D` binding and retire its KDE service shortcut.

## Consequences

- No `input` group and no `sudo` for input — only `/dev/uinput` write (already granted by the
  session ACL). Smaller attack surface than evdev.
- The hotkey now depends on a KWin global-shortcut binding, set up by the installer/docs (T6);
  the binding, not the app, owns the key. Acceptable given the KDE-only scope.
- One short-lived forwarder process per keypress (single-instance), far cheaper than
  PathofTrading's per-press Python launch; the main process stays warm.
- `evdev` remains a dependency only for its `uinput` virtual-device API; the device-reading code
  is removed.
