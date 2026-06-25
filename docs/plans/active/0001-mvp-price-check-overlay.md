---
status: active
created: 2026-06-23
updated: 2026-06-23
adrs: [1, 2, 3]
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
- [x] **T3** — Input path: global hotkey → `uinput` Ctrl+C synth → X11 clipboard read. **Pivoted to a KDE global shortcut (ADR-0002), not evdev:** evdev can read keys but not *consume* them, so a chord on a game-bound key (PoE2's `D`) leaks to the game and moves the character (confirmed: Alt+D moved the character). A KWin global shortcut intercepts the chord first — the proven PathofTrading mechanism. **Impl (code-complete; in-game test pending):** `Ctrl+Alt+D` is a KDE service shortcut running `poe2-overlay --price-check`; `tauri-plugin-single-instance` (first plugin) forwards it to the running app → `hotkey::price_check` synthesizes Ctrl+C via a warm `uinput` device (`Synth` in Tauri state) → 120 ms → reads the X11 CLIPBOARD selection (`x11-clipboard` crate, no `xclip` binary) → emits `price-check-item` + shows the overlay; ✕/Esc hides. No `/dev/input` reads, so **no `input` group / sudo** — only the `/dev/uinput` session ACL. Verified end-to-end minus the game: second-instance forwarding fires `price_check`, synth device builds, clipboard read returns text (27 chars seeded). Took over PathofTrading's `Ctrl+Alt+D` (kglobalshortcutsrc + a `poe2-overlay-pricecheck.desktop`; backup saved). **Input-trap incident + fix ([ADR-0003]):** the first in-game trigger locked the machine out — `window.show()` mapped the **full-output** T2 surface, whose `wl_surface` input region (CSS `pointer-events:none` does not shrink it) swallowed every click with only an on-screen ✕/Esc to escape; hard-restart required. Fixed by making the surface a **fixed-size sub-output rectangle** (`set_size_request` **+ `resize(1,1)`** — tao pre-pins the window to the conf size 1140×600, so the `resize` is what commits the real size; corrects T2's "full-output required", which was just a missing size request) so it can never cover the screen, plus a compositor-level **`Ctrl+Alt+X` → `--hide`** shortcut (single-instance forwarding, ADR-0002) as a guaranteed escape. Adversarially audited — no residual lockout. **Then three functional bugs, fixed in order:** (1) **repeat checks read an empty clipboard** — *not* the focus theory first chased (KeyboardMode/GTK focus props made no difference); the real cause is an **XWayland clipboard read-race** — the game's copy reaches the X11 selection only after KWin's sync, and a single read at 120 ms catches a transient-empty mid-sync state. Fixed by **polling** the clipboard (~40 ms × up to 20, ~800 ms) until non-empty, matching PathofTrading's retrying backend. (2) **stacked ghost popups** — a content-sized card shrinks for shorter items and WebKitGTK leaves the previously-painted transparent region uncleared until a later repaint. Fixed with a **fixed-size 400×380 card** that overpaints the same region every time, plus a `show()`-only-when-hidden guard. (3) **panel moved to screen-centre** (per user; the corner was too far to glance at) — surface unanchored+centred 440×420. `KeyboardMode::None` kept on its own merit (a game overlay must not steal the keyboard from PoE2). **Done (user-confirmed in-game):** hover item + Ctrl+Alt+D shows the centred price card; repeat checks replace it cleanly; ✕/Ctrl+Alt+X dismiss; no lockout. **Follow-ups (T6):** `Ctrl+Alt+X` needs the live `kglobalacceld` to reload (relogin) to fire; and a first launch with no running instance drops its own `--price-check`/`--hide` flag (the single-instance callback fires only for the 2nd instance) and leaves a stray hidden process holding `/dev/uinput` — fold into autostart.
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
- T3 input mechanism pivoted evdev → KDE global shortcut ([ADR-0002]). evdev can't
  consume keys, so a game-bound chord (PoE2 `D`) leaks and moves the character. The
  KDE shortcut consumes the chord and removes the `input`-group requirement. `xclip`
  (ADR-0001's pick) isn't installed; we read the X11 selection in-process via the
  `x11-clipboard` crate — no external binary, same X11 selection.
- T3 copy is plain Ctrl+C (PoE2 basic copy, confirmed by PathofTrading's
  run_pricecheck.sh). Advanced item text (Ctrl+Alt+C) is a one-line synth change
  once basic pricing works.
- **Overlay surface model: full-output → sized, centred, focus-free ([ADR-0003],
  supersedes the T2 full-output decision above).** The full-output surface trapped all
  screen input (CSS `pointer-events:none` does not shrink a `wl_surface` input region)
  and locked the user out → hard restart. The surface is now a fixed-size unanchored
  (centred) rectangle, sized with the gtk-layer-shell two-call idiom
  (`set_size_request(440,420)` **then** `resize(1,1)` — tao pre-pins the window to the
  conf size 1140×600 and `set_size_request` only raises the minimum, so the `resize`
  is what commits the real size; T2's "collapse-to-0" was simply a missing size
  request). It cannot cover the screen. Plus **`KeyboardMode::None`** (a game overlay
  must not steal the keyboard from PoE2) and a compositor-level `Ctrl+Alt+X` → `--hide`
  escape. Per-region click-through *within* the panel stays the T5 seam.
- **Repeat-checks empty clipboard = XWayland read-race, NOT focus (dead-end recorded so
  it is not re-chased).** First theory was focus theft (overlay grabs focus → clipboard
  bridge empties); `KeyboardMode::None`, hide-before-copy, and GTK `accept_focus`/
  `focus_on_map(false)` ALL failed to change the symptom. Real cause: the game's copy
  reaches the X11 CLIPBOARD only after KWin's XWayland sync, and a single read at 120 ms
  catches a transient-empty state (proven by the diagnostic that the item appeared on
  the *next* press's pre-read). Fix: **poll** the clipboard (~40 ms × up to 20) until
  non-empty — the same retry PathofTrading's backend does. The speculative focus props
  were removed in the cleanup; `KeyboardMode::None` stays for the keyboard reason above.
- **Stacked ghost popups = WebKit transparent-repaint, fixed by a constant-size card.**
  A content-sized card shrinks for shorter items; WebKitGTK does not clear the
  previously-painted transparent region until a later repaint, so old cards linger
  stacked (they clear "after some time"). Fix: a **fixed 400×380 card** that overpaints
  the same region each update, plus `show()` only when the window is hidden (calling it
  on an already-mapped layer surface was a suspected second cause; the fixed size was
  the actual fix).

[ADR-0002]: ../adr/0002-kde-global-shortcut-hotkey.md
[ADR-0003]: ../adr/0003-overlay-dismissal-safety-corner-surface-hide-shortcut.md
