# PoE2 Linux Overlay — Research & Decision

_Workbench research, 2026-06-23. Goal: a PoE2 trade overlay (à la Overwolf's
"PoE Overlay II") that actually works on **this** machine: CachyOS, native
Wayland + KWin (KDE Plasma 6), PoE2 via Steam Proton (appid 2694490)._

## The target we're replicating

**PoE Overlay II** (Kyusung4698) on Overwolf — hotkey price-check + trade/whisper
panel + market browser + map/atlas danger-checker + item inspect + regex/cheat
sheets + leveling guide. Data is **out-of-band** (clipboard item-text + GGG trade
API + `Client.txt` tailing — no memory reading, ToS-acceptable). It's Windows-only
because it composites by **hooking the game's DirectX present call via DLL
injection** — no Linux equivalent exists, so a port is impossible; a Linux build
is a re-architecture.

## Root cause of past failures (confirmed)

Every *popular* option — **Exiled-Exchange-2**, **awakened-poe-trade**,
**Sidekick** — draws its overlay with `electron-overlay-window`, which is
**X11-only** (tracks the game window via X11/EWMH). On native KWin Wayland that
breaks exactly how it broke for us: won't composite over the Proton game, wrong
monitor, or steals input. Our exact stack is reported broken in EE2 issues
[#816], [#817], [#673]; the only "fix" (force everything to XWayland) still fails
for many CachyOS+KDE users. **These are dead ends.**

[#816]: https://github.com/Kvan7/Exiled-Exchange-2/issues/816
[#817]: https://github.com/Kvan7/Exiled-Exchange-2/issues/817
[#673]: https://github.com/Kvan7/Exiled-Exchange-2/issues/673

## The right mechanism

A **wlr-layer-shell** surface on the **OVERLAY** layer (KWin confirmed Oct 2025
these render above fullscreen clients), click-through via empty input region,
hotkeys via the XDG GlobalShortcuts portal or raw evdev. Game runs Proton/XWayland;
items copied via ydotool/uinput → clipboard. This is what the working repos do.

## Candidate matrix

| Project | Stack | Works on our stack? | Verdict |
|---|---|---|---|
| **PathofTrading** (brendancohan) | Python + QML/Quickshell, wlr-layer-shell | ✅ **Confirmed** — issue #2 is a CachyOS 7.0.11 + KDE 6.6 user, works | **Install & use now** |
| **ExileWatch** (5h4rkByt3) | **Rust** + Tauri + gtk-layer-shell, evdev+uinput+xclip | Mechanism real, author-only tested | **Best fork base (our language)** |
| **LP0101 EE2 fork** (`wayland` branch) | Electron + KWin-script + KGlobalAccel + ydotool | Built for KDE6+Proton; author-only tested; "delicate" | Fork for **full feature set** |
| **Waystone** (kriskruse) | Python+GTK4 / Node, gtk4-layer-shell + xdg portal | ❌ Hardcoded Hyprland (issue #13 = our stack, broken) | Fork base / reference |
| sergiulache/poe-overlay-linux | Python+GTK4, gtk4-layer-shell | PoE1 leveling only, no PoE2 | Reference only |
| Kvan7 EE2 / SnosMe APT / Sidekick | Electron/.NET, X11-only overlay | ❌ Dead on native Wayland | Avoid |

Repos: PathofTrading https://github.com/brendancohan/PathofTrading ·
ExileWatch https://github.com/5h4rkByt3/ExileWatch ·
LP0101 https://github.com/LP0101/Exiled-Exchange-2/tree/wayland ·
Waystone https://github.com/kriskruse/Waystone

## Decision (2026-06-23)

1. **Validate first:** install **PathofTrading** (confirmed working on this exact
   stack) — proves the layer-shell + ydotool loop on our hardware. See `INSTALL.md`.
2. **Then build our own**, scope = **price-check + map/atlas + regex helpers**.
   Leading fork base: **ExileWatch** (Rust/Tauri, correct native mechanism, our
   primary language); mine LP0101 for feature breadth + the KWin-script trick, and
   Waystone for its xdg-portal GlobalShortcuts path.

## Security note

PathofTrading was audited (clean — not malware; network only to
`www.pathofexile.com` + `poe.ninja`; no credential reads, no root, no persistent
service, no writes outside `~`). Caveats: (RF-1) the hotkey fires a kernel Ctrl+C
into **whatever window is focused** — there is no game-focus check; use bind-on-
release. (RF-4) don't trigger it with a secret on the clipboard — the fallback path
sends clipboard text to GGG's trade search. All forks here are tiny, solo, partly
LLM-assisted — audit before building on them.

## The one unverified risk for all candidates

Whether KWin composites a layer-shell OVERLAY surface over **exclusive** fullscreen.
Test both windowed/borderless-fullscreen and exclusive fullscreen in-game.
