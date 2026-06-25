# PoE2 Overlay

A Wayland-native **Path of Exile 2 trade overlay** for **KDE Plasma** (Tauri 2 +
Rust + Vue). It draws over a fullscreen Proton game via `wlr-layer-shell`, copies
the hovered item, and prices it against the GGG trade2 API + poe.ninja — the same
job the Windows-only Overwolf "PoE Overlay II" does, but built for native KWin
Wayland where the X11-based overlays (Exiled-Exchange-2, awakened-poe-trade) fail.

> Status: **price-check MVP working** (T1–T6). Layer-shell overlay over fullscreen
> Proton, KDE-global-shortcut input path, pricing core (poe.ninja bulk + GGG trade2
> gear), overlay UI (listings, filter toggles, league selector, requery), and
> packaging/install. Tracked in `docs/plans/active/`; map/atlas + regex helpers (T7–T8)
> are post-MVP.

## Why this exists / how it works

See `docs/adr/` for the decisions (ADR-0001 the clean build + layer-shell mechanism,
0002 the KDE-shortcut input path, 0003 overlay dismissal safety, 0004 the pricing core
+ event contract) and `docs/research/RESEARCH.md` for the landscape study (why X11
overlays break on Wayland, the candidate matrix). `CONTEXT.md` is the glossary;
`docs/research/INSTALL.md` covers the validated reference, **PathofTrading**.

Target stack: CachyOS / KDE Plasma 6 / KWin Wayland, PoE2 via Steam Proton.

## Build prerequisites (Arch/CachyOS)

```bash
sudo pacman -S --needed webkit2gtk-4.1 gtk3 gtk-layer-shell base-devel
# Rust + Node also required; the in-game copy needs /dev/uinput access
# (session ACL or the `input` group).
```

## Install (KDE Plasma 6 Wayland)

```bash
npm install            # first time
npm run tauri:build    # -> AppImage/deb/rpm under src-tauri/target/release/bundle/
./packaging/install.sh # binary + KDE shortcuts (Ctrl+Alt+D / Ctrl+Alt+X) + autostart
```

Then log out and back in (so KWin registers the shortcuts and the overlay autostarts
warm). Full runbook — uinput access, key customization, troubleshooting, uninstall —
in **`docs/INSTALL.md`**.

## Develop

```bash
npm install
npm run tauri:dev   # or: npm run tauri dev
```
