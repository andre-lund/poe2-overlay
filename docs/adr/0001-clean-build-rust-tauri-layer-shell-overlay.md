---
status: accepted
---

# 0001 — Build a Wayland-native PoE2 trade overlay as a clean Rust/Tauri layer-shell app (ExileWatch as reference, not a fork)

**Implemented by:** [docs/plans/active/0001-mvp-price-check-overlay.md](../plans/active/0001-mvp-price-check-overlay.md)
**Partly superseded by:** [ADR-0002](0002-kde-global-shortcut-hotkey.md) — the `evdev` global-hotkey decision is replaced by a KDE global shortcut; the rest of this ADR stands.

We build our own Path of Exile 2 trade overlay for KDE Plasma / KWin Wayland as a
fresh Rust + Tauri 2 + Vue project. The overlay is drawn as a `wlr-layer-shell`
surface on the OVERLAY layer; item copy uses an `evdev`/`uinput` synthesized
keypress with a clipboard read; pricing hits the GGG trade2 API + poe.ninja. We
treat ExileWatch, PathofTrading, and Waystone as references and reimplement —
we do not fork any of them.

## Context

On native KWin Wayland the popular overlays (Exiled-Exchange-2, awakened-poe-trade,
Sidekick) fail: their `electron-overlay-window` mechanism is X11-only and cannot
draw over a fullscreen Proton game, lands on the wrong monitor, or steals input —
confirmed on this exact CachyOS+KDE+Proton stack (see `docs/research/RESEARCH.md`).
The mechanism that works is a `wlr-layer-shell` OVERLAY-layer surface, proven on
this machine by PathofTrading (Quickshell) and demonstrated in Rust by ExileWatch
(gtk-layer-shell). PathofTrading's per-keypress Python process also makes pricing
feel slow (no warm HTTP/DNS between checks; ~200ms fixed overhead is dwarfed by
the GGG round-trip). The strongest Rust reference, **ExileWatch, ships no license
(all rights reserved)** — so forking its tree is not permitted; only its
techniques (uncopyrightable) may inform our own code.

## Decision

- **Clean build, not a fork.** Fresh project we own and license ourselves;
  ExileWatch / PathofTrading (GPLv3) / Waystone (AGPL) are read as references only.
- **Stack:** Rust + Tauri 2 + Vue/Vite (WebKitGTK on Linux), matching the proven
  ExileWatch architecture and the user's primary language.
- **Overlay mechanism:** promote the Tauri GTK window to a `wlr-layer-shell`
  surface on the OVERLAY layer (KWin composites it above fullscreen); click-through
  via an empty input region; on-demand keyboard interactivity.
- **Input:** global hotkey via raw `evdev` (compositor-focus-independent),
  in-game copy synthesized via a `uinput` virtual device, clipboard read
  preferring the X11 selection (`xclip`) over `wl-paste` (which returns KWin's
  stale clipboard for XWayland/Proton clients).
- **Pricing:** bulk/stackables via poe.ninja (zero GGG quota); gear/waystones via
  the GGG trade2 search+fetch API, honoring `X-Rate-Limit` headers.
- **Persistent app** (not process-per-keypress) so the HTTP client + DNS stay warm
  between checks — the latency win over PathofTrading.
- **Required env:** `GDK_BACKEND=wayland` and `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  set before GTK/WebKit init.

## Consequences

- We own the code and can license it as we choose; no upstream license risk.
- More upfront work than a `git clone` fork, but the target scope (price-check +
  map/atlas + regex) diverges enough that most code would be ours regardless.
- Build depends on system libs: `webkit2gtk-4.1`, `gtk3`, `gtk-layer-shell`;
  input needs `/dev/uinput` access (session ACL or `input` group).
- Layer-shell over *exclusive* fullscreen is unverified on KWin; borderless /
  windowed-fullscreen is the known-good config (tracked in the plan).
- Map/atlas danger-checker and regex helpers are in scope but sequenced after the
  price-check MVP.
