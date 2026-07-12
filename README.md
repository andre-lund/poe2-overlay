# PoE2 Overlay

A Wayland-native **Path of Exile 2 trade overlay** for **KDE Plasma** (Tauri 2 +
Rust + Vue). It draws over a fullscreen Proton game via `wlr-layer-shell`, copies
the hovered item, and prices it against the GGG trade2 API + poe.ninja — the same
job the Windows-only Overwolf "PoE Overlay II" does, but built for native KWin
Wayland where the X11-based overlays (Exiled-Exchange-2, awakened-poe-trade) fail.

> Status: **feature-complete, in-game verification pending** (T1–T11). Everything
> below is code-complete and reviewed; T1–T3 are user-confirmed in-game, T4–T11 await
> the final in-game pass before the plan archives. Tracked in `docs/plans/active/`.
> T11 restyled the overlay to PoE2's own tooltip look (Fontin, bronze/gold, rarity
> colors) and moved to on-demand keyboard focus so the typed filters work (ADR-0007).

## What it does

Three hotkeys, all forwarded to one warm background instance (KDE global shortcuts):

- **`Ctrl+Alt+D` — price check.** Hover an item, press the key: the overlay copies it,
  parses the item text, and prices it — gear via a GGG trade2 search+fetch (per-stat
  filter toggles, editable min/max, league selector, explicit requery), currency and
  bulk via poe.ninja (zero GGG quota). Self-throttles against GGG's `X-Rate-Limit`
  headers so a mashed hotkey can't get you IP-banned.
- **`Ctrl+Alt+D` over a Waystone — danger check.** A waystone yields a danger verdict
  instead of a price: a Safe/Caution/Dangerous/Deadly rating with the specific map mods
  that earned it, from a keyword ruleset grounded in the live PoE2 mod surface. Local,
  instant, no GGG quota.
- **`Ctrl+Alt+F` — price sheet.** A browsable poe.ninja catalogue for the reward panels
  the game won't let you copy from (runes, uncut/lineage gems, currency, fragments,
  essences, uniques, tablets…), grouped under General / Equipment / Atlas tabs with a
  client-side filter. One round-trip per category switch.
- **`Ctrl+Alt+X` — panic hide.** Guaranteed compositor-level escape that dismisses the
  overlay (also `✕` / `Esc`).

A regex stash/vendor cheat-sheet (clipboard-write) is built but **dormant** — its
`Ctrl+Alt+F` slot now serves the price sheet; see ADR-0006.

## Why this exists / how it works

See `docs/adr/` for the decisions (ADR-0001 the clean build + layer-shell mechanism,
0002 the KDE-shortcut input path, 0003 overlay dismissal safety, 0004 the pricing core
+ event contract, 0005 the waystone danger check, 0006 the regex cheat-sheet) and
`docs/research/RESEARCH.md` for the landscape study (why X11 overlays break on Wayland,
the candidate matrix). `CONTEXT.md` is the glossary; the validated reference,
**PathofTrading**, is studied in `docs/research/`.

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
./packaging/install.sh # binary + KDE shortcuts (Ctrl+Alt+D / Ctrl+Alt+X / Ctrl+Alt+F) + autostart
```

Then log out and back in (so KWin registers the shortcuts and the overlay autostarts
warm). Full runbook — uinput access, key customization, troubleshooting, uninstall —
in **`docs/INSTALL.md`**.

## Develop

```bash
npm install
npm run tauri:dev   # or: npm run tauri dev
```

## License & attribution

**GPL-3.0-or-later** (see `LICENSE`). The pricing core reimplements techniques from
[PathofTrading](https://github.com/brendancohan/PathofTrading) by brendancohan
(GPLv3) — studied as the validated reference on this exact stack and translated to
Rust, so this project carries the GPL lineage forward. The reference itself is
vendored unmodified (license and headers intact) under `eval-pathoftrading/` for
provenance. ExileWatch and other overlays were studied as *behavioral* references
only (see ADR-0001); no code from them is included.

Not affiliated with Grinding Gear Games. The overlay uses the public trade2 API and
poe.ninja, and self-throttles against GGG's `X-Rate-Limit` headers (ADR-0004).
