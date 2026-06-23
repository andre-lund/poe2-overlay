---
status: active
created: 2026-06-23
updated: 2026-06-23
adrs: [1]
---

# Plan: MVP price-check overlay (then map/atlas + regex)

## Intent

Deliver a working PoE2 trade overlay on KDE Wayland: press a hotkey over an
in-game item and see live pricing in an overlay drawn on top of the game. This
is the floor that makes the project useful; the map/atlas danger-checker and
regex helpers build on the same overlay + input + data foundation afterward.

## Approach

Per [ADR-0001](../adr/0001-clean-build-rust-tauri-layer-shell-overlay.md): a Rust
+ Tauri 2 + Vue app whose GTK window is promoted to a `wlr-layer-shell` OVERLAY
surface; input via `evdev` (hotkey) + `uinput` (synthesized copy) + clipboard;
pricing via poe.ninja (bulk) and the GGG trade2 API (gear). The Tauri shell is
scaffolded; module seams exist in `src-tauri/src/{overlay,hotkey,trade}.rs`.
Reference implementations: ExileWatch (Rust/gtk-layer-shell), PathofTrading
(Quickshell, validated on this machine), Waystone (portal GlobalShortcuts) —
read, do not copy.

The make-or-break unknown is whether KWin composites the layer-shell OVERLAY
surface over *exclusive* fullscreen; T2 must test both fullscreen modes and
record the result (borderless is the known-good fallback).

## Tasks

- [ ] **T1** — Build prereqs + confirm the empty Tauri shell runs: `pacman -S webkit2gtk-4.1 gtk3 gtk-layer-shell`, `npm install`, `npm run tauri:dev` opens the (hidden) window without error.
- [ ] **T2** — Layer-shell overlay surface: add `gtk-layer-shell`/`gdk`, promote the main window (Layer::Overlay, anchors, empty input region for click-through, on-demand keyboard), show/hide. **Verify over a real Proton PoE2 game in BOTH borderless and exclusive fullscreen.**
- [ ] **T3** — Input path: `evdev` global hotkey + `uinput` Ctrl+Alt+C (PoE2 advanced-copy) + clipboard read preferring `xclip` (X11 selection) over `wl-paste`.
- [ ] **T4** — Pricing core: parse PoE2 item text; bulk via poe.ninja, gear via GGG trade2 search+fetch with `X-Rate-Limit` handling; persistent warm HTTP client.
- [ ] **T5** — Overlay UI (Vue): listings, per-stat filter toggles, requery, league selector; transparent/click-through styling.
- [ ] **T6** — Package + launch: AppImage build, KDE global shortcut + `ydotoold`-equivalent / uinput setup docs, autostart.
- [ ] **T7** — (post-MVP) Map/atlas danger-checker: flag dangerous waystone/map mod combinations.
- [ ] **T8** — (post-MVP) Regex helpers: stash/vendor search regex builder + cheat-sheets.

## Decision log

- Scope split: T1-T6 = price-check MVP; T7-T8 (map/atlas + regex) are in project
  scope but sequenced after the MVP works end-to-end.
- gtk-layer-shell/evdev/reqwest are declared (commented) in `Cargo.toml` and
  activated per task to keep the scaffold buildable before system libs are present.
