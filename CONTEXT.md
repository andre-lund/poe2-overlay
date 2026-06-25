# PoE2 Overlay

A Wayland-native Path of Exile 2 trade overlay for KDE Plasma. It draws over a
fullscreen Proton game using a `wlr-layer-shell` surface, reads the hovered item
by synthesizing an in-game copy and reading the clipboard, and prices it against
the GGG trade2 API and poe.ninja. It is a clean Rust + Tauri 2 + Vue build,
informed by ExileWatch / PathofTrading / Waystone as references (not a fork).

<!-- CONTEXT.md is the single authoritative glossary for this repo (the
     project-docs standard). One entry per term of art; add as terms emerge. -->

## Language

**Overlay**:
The layer-shell surface the app draws over the game. It is a real Wayland
`wlr-layer-shell` surface on the OVERLAY layer, not an ordinary top-level window.
_Avoid_: "window", "HUD", "popup".

**Layer-shell**:
The `wlr-layer-shell` (`zwlr_layer_shell_v1`) protocol on the OVERLAY layer —
the Wayland-native mechanism KWin composites above a fullscreen client. The
reason this works where X11 always-on-top overlays fail on native Wayland.
_Avoid_: "always-on-top window", "X11 overlay", "override-redirect".

**Price check**:
The end-to-end flow: hotkey → synthesize copy into the game → read clipboard →
parse item → query pricing → render listings in the overlay.

**Bulk vs gear**:
Two pricing paths. **Bulk** = stackable/currency items priced via poe.ninja
(zero GGG quota). **Gear** = rares/uniques/waystones priced via the GGG trade2
search+fetch API (rate-limited).
_Avoid_: "exchange" for gear; "search" for bulk.

**Proton game**:
PoE2 running under Steam Proton — an XWayland client from the compositor's view.
The overlay is a native Wayland client composited on top of it.

**Waystone**:
A PoE2 map item (item class `Waystones`); its affix mods determine the dangers of the
area it opens. Parsed through the gear path (mods land in the item's stat list), but
**not priced** — a price-check on a waystone runs the danger-check instead.
_Avoid_: "map" (reserve for the area/instance), "scarab", "sextant".

**Danger-check**:
The waystone analysis path: the price-check hotkey, on a waystone, matches its mods
against a build-agnostic ruleset and emits a danger verdict (`price-check-danger`,
ADR-0005) — a [`DangerLevel`] (Safe/Caution/Dangerous/Deadly) plus the matched
dangers — instead of pricing. Local, instant, zero GGG quota.
_Avoid_: "price" for a waystone; "mod roll" for the danger flags.
