---
status: draft
created: 2026-07-12
updated: 2026-07-12
adrs: [1, 2, 4, 7]
---

# Plan: Windows port

## Intent

Bring the overlay to Windows for reach (friends, releases) while keeping the
CachyOS/KDE Wayland build first-class. The Linux version exists because X11 overlays
fail on native Wayland (ADR-0001); on Windows that gap is already served by Awakened
PoE Trade / Exiled Exchange 2 — the case for porting is *this* overlay's
differentiators: the waystone danger checker (ADR-0005) and the category price sheet
(T9/T10), which no existing Windows overlay offers. The pricing core, danger/cheat
data, and the whole Vue UI + PoE2 theme port untouched; only the three platform seam
modules need Windows counterparts.

## Approach

The platform-specific surface is already isolated in exactly three files —
`overlay.rs` (layer-shell promotion), `hotkey.rs` (KDE-shortcut forwarding + uinput
synth + `wl-paste`), `clipboard.rs` (X11 write) — plus `packaging/`. Strategy:
`#[cfg]`-split those seams, keep the event contract (ADR-0004) and the frontend
byte-identical, and lean on the fact that Windows makes each seam *simpler*:

- **Overlay window:** no layer-shell — a transparent, undecorated, `alwaysOnTop`,
  fixed-size, centered Tauri window is native on Windows (WebView2). Same
  hidden-until-triggered lifecycle. Constraint carried from every Windows overlay:
  the game must run **borderless/windowed fullscreen** (exclusive fullscreen
  occludes overlays) — documented, not worked around.
- **Hotkeys:** `RegisterHotKey` consumes chords natively, so the entire
  KDE-shortcut → `--price-check` → single-instance forwarding architecture
  (ADR-0002) collapses to the Tauri global-shortcut plugin registering
  Ctrl+Alt+D/F/X in-process on Windows. Linux keeps the ADR-0002 path unchanged.
- **Copy synth + clipboard:** uinput/ydotool → `SendInput` (via `enigo`);
  `wl-paste`/x11-clipboard → `arboard` for both read and write. The
  clipboard-change polling loop and same-item re-check logic in `price_check` stay
  shared — only the read/write/synth primitives swap.
- **Packaging:** `tauri build` already emits NSIS on Windows; autostart via the
  Tauri autostart plugin; builds + releases via a GitHub Actions windows runner
  (no cross-compile gymnastics). Fontin cannot be redistributed (see DESIGN.md), so
  the installer-equivalent fetch step needs a Windows form (first-run download into
  the user's font store, or a documented manual step).

If the Windows window/input mechanism decisions harden into anything
non-obvious during T1, promote them to ADR-0008 rather than growing this plan.

Open questions (gate: resolve before the implementing task starts):

- [NEEDS CLARIFICATION: test hardware — is there a Windows machine/VM with PoE2
  installed to iterate on? T2/T3/T5 are blocked on real hardware; WebView2 + hotkey
  behavior can't be verified from Linux.]
- [NEEDS CLARIFICATION: keep Ctrl+Alt+D / Ctrl+Alt+F / Ctrl+Alt+X as the Windows
  chords, or rebind (e.g. Ctrl+D like Awakened PoE Trade uses)? Configurability
  scope?]
- [NEEDS CLARIFICATION: distribution — GitHub Releases from an Actions windows
  runner, or local builds only?]

## Tasks

- [ ] **T1** — Seam split: move the Linux implementations of `overlay.rs`,
  `hotkey.rs` (synth + clipboard read), and `clipboard.rs` behind
  `#[cfg(target_os = "linux")]` module fronts with a shared trait/function surface;
  stub Windows counterparts. Verify: Linux `cargo test` + `clippy` + in-game smoke
  unchanged; `cargo check --target x86_64-pc-windows-msvc` compiles the stubs.
  Promote mechanism decisions to ADR-0008 if warranted.
- [ ] **T2** — Windows overlay window: transparent/undecorated/alwaysOnTop
  fixed-size centered window, hidden-until-triggered, show/hide parity with the
  layer-shell path. Verify: renders the themed card over borderless-fullscreen PoE2
  on Windows; no taskbar entry; `Ctrl+Alt+X`-equivalent hides it.
- [ ] **T3** — Windows input path: global-shortcut plugin chords (consumed, no
  character leak into the game), `SendInput` Ctrl+C synth, `arboard`
  clipboard read/write; two-phase pricing contract end-to-end. Verify: hover +
  hotkey prices a real item in-game; regex-copy paste works in the game's Ctrl-F box.
- [ ] **T4** — Packaging + release: NSIS bundle, autostart plugin wiring, Windows
  Fontin fetch step, GitHub Actions windows build. Verify: clean-machine install →
  hotkeys work after reboot; Actions artifact installs.
- [ ] **T5** — Windows in-game verification pass (the T4–T11-style gate): pricing +
  requery + typed filters, waystone danger, price sheet tabs, rate-limit lockout
  behavior, panic hide. Verify: user-confirmed in-game on Windows.

## Decision log

- 2026-07-12 — Plan drafted post-0001-archive. Windows chosen as a port target for
  reach, not necessity (the Windows gap is already served; the differentiators are
  the danger checker + price sheet). Linux remains the primary target.
