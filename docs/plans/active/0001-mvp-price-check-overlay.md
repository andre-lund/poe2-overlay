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

- [x] **T1** — Build prereqs + confirm the empty Tauri shell runs. **Done:** all Tauri Linux build deps already present (webkit2gtk-4.1, gtk3, base-devel, openssl, librsvg, libappindicator-gtk3) — no sudo install needed; `npm install` + `npm run build` pass; `cargo build` clean (4 expected stub warnings); `npm run tauri:dev` launches the hidden window (vite :1420, `Running target/debug/poe2-overlay`, no panic — only a benign `libx265` GStreamer plugin-scan warning). Added `tauri:dev`/`tauri:build` scripts setting `GDK_BACKEND=wayland` + `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
- [x] **T2** — Layer-shell overlay surface: add `gtk-layer-shell`/`gdk`, promote the main window (Layer::Overlay, anchors, on-demand keyboard), show/hide. **Verify over a real Proton PoE2 game.** **Impl:** activated `gtk = "0.18"` + `gtk-layer-shell = { "0.8", features = ["v0_6"] }` (matches Tauri 2.11's gtk-rs 0.18; `v0_6` gates `KeyboardMode`). `overlay::init_layer_shell` promotes the still-hidden GTK window — `Layer::Overlay`, **all-four-edge anchors (full-output surface)**, `exclusive_zone(-1)`, `KeyboardMode::OnDemand`, namespace `poe2-overlay`. The card is positioned top-right via CSS within the full-screen canvas; `hide_overlay` command unmaps it; ✕ button + Esc dismiss. **Make-or-break CONFIRMED:** the full-output overlay composites over fullscreen Proton PoE2 — the user saw the popup drawn on top of the running game. **Two corrected dead-ends:** (1) full-output + `set_ignore_cursor_events` click-through trapped *all* input — tao sets the input-shape on the *toplevel* GDK window, which the WebKitGTK child surface ignores, so the transparent screen ate clicks with no exit (user-reported); fix = a ✕/Esc-dismissable modal (PathofTrading's full-screen, focusable, show-on-demand model) rather than click-through. (2) a corner-sized surface (top+right, 2 anchors) collapses to ~0 size — gtk-layer-shell takes the size from the WebKitGTK child whose min-size request is ~0, so nothing renders (user saw nothing); **full-output (4 anchors) is required** to force a non-zero size. (`Exclusive` keyboard focus also rejected — it drops the game out of fullscreen.) Compiles clean (only T3/T4 stub warnings); surface maps (instrumented `map-event` + `size-allocate`). **Tooling caveat:** `spectacle`/KWin screencast does not reliably capture the transparent overlay layer over a fullscreen game on this HDR setup (captured it once on a clean desktop, not since) — visual confirmation is by eye, not screenshot. **Done (user-confirmed):** the card shows top-right composited over fullscreen PoE2 and ✕/Esc dismisses. While shown it is modal (covers the screen); T3 hides it by default + shows on the hotkey.
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
- T2 overlay model: corner-sized surface (top+right, 460×160), hidden-by-default,
  shown on demand, dismissable — matching the proven PathofTrading reference. NOT a
  full-output click-through canvas: `set_ignore_cursor_events` sets the input-shape on
  the toplevel GDK window, which the WebKitGTK child surface ignores, so a full-output
  surface eats every click. Confirmed compositing over fullscreen PoE2 (user saw the
  corner popup over the running game), so the make-or-break is settled for this stack.
- T2 fallback (if a future fullscreen mode ever hides it): borderless/Windowed
  Fullscreen is the PathofTrading-validated config; exclusive fullscreen could in
  principle trigger a KWin direct-scanout bypass. Diagnostic lever if so:
  `KWIN_DRM_NO_DIRECT_SCANOUT=1` in the game env, or System Settings → Display &
  Monitor → Compositor "Allow applications to block compositing".
- T2 tooling note: `spectacle`/KWin screencast does not reliably capture a transparent
  layer-shell surface composited over fullscreen (returns a transparent frame). Verify
  the overlay by eye, not by screenshot.
- T5 will decide the real panel: a larger sized surface still has transparent dead
  zones that catch clicks within its bounds; per-region input (or content-sized
  surface) is the T5 seam — not needed for the corner probe.
