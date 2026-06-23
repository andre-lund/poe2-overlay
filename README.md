# PoE2 Overlay

A Wayland-native **Path of Exile 2 trade overlay** for **KDE Plasma** (Tauri 2 +
Rust + Vue). It draws over a fullscreen Proton game via `wlr-layer-shell`, copies
the hovered item, and prices it against the GGG trade2 API + poe.ninja — the same
job the Windows-only Overwolf "PoE Overlay II" does, but built for native KWin
Wayland where the X11-based overlays (Exiled-Exchange-2, awakened-poe-trade) fail.

> Status: **scaffold**. The Tauri shell is in place; the overlay surface, input,
> and pricing are stubbed and tracked in `docs/plans/active/`.

## Why this exists / how it works

See `docs/adr/0001-*.md` for the decision and `docs/research/RESEARCH.md` for the
full landscape study (why X11 overlays break on Wayland, the candidate matrix, the
layer-shell mechanism). `docs/research/INSTALL.md` documents the validated working
reference, **PathofTrading**.

Target stack: CachyOS / KDE Plasma 6 / KWin Wayland, PoE2 via Steam Proton.

## Build prerequisites (Arch/CachyOS)

```bash
sudo pacman -S --needed webkit2gtk-4.1 gtk3 gtk-layer-shell base-devel
# Rust + Node already required; global-hotkey/uinput needs /dev/uinput access
# (session ACL or the `input` group).
```

## Develop

```bash
npm install
npm run tauri:dev   # or: npm run tauri dev
```
