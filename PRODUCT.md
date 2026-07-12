# Product

## Register

product

## Users

One user: a PoE2 player on KDE Plasma Wayland, mid-map with the game fullscreen. The
overlay appears for a few seconds at a time — price-check an item, read a waystone
verdict, scan the price sheet — then disappears. Ambient light is a dark room lit by
the game scene; the panel sits over bright, chaotic gameplay.

## Product Purpose

A Wayland-native trade overlay for Path of Exile 2 (Tauri + Rust + Vue,
wlr-layer-shell). It copies the hovered item, prices it against the GGG trade2 API
and poe.ninja, flags dangerous waystone mods, and shows a browsable price sheet.
Success = the answer is readable in under two seconds without leaving the game.

## Brand Personality

In-world, quiet, trustworthy. The panel should read as part of PoE2's own UI — the
item-tooltip vocabulary players already parse hundreds of times per session: black
glass, bronze/gold chrome, serif display, rarity colors. The tool disappears into
the game, not into a generic desktop app.

## Anti-references

- Slick modern SaaS overlay: blue accents, Inter, glassy rounded cards (the current
  look — explicitly rejected).
- Overwolf-style ad-chrome overlays.
- Anything that washes out over a bright game scene (low-opacity panels failed in T3).

## Design Principles

- **Native to the game, not the desktop.** Reuse PoE2's visual vocabulary (rarity
  colors, gold-on-black, tooltip separators) so zero learning is needed.
- **Readable over chaos.** Solid dark backdrops, high contrast text; the game scene
  behind is the enemy of legibility.
- **Glanceable verdicts.** Price, spread, danger level land in the first second;
  detail (filters, listings, ages) is secondary.
- **Never fight the game.** No keyboard stealing, no input traps, no decoration that
  costs frame time (WebKitGTK repaints are a real constraint — ADR-0003).

## Accessibility & Inclusion

Body text ≥ 4.5:1 against the panel; danger severity never encoded by color alone
(label text always present). Reduced-motion honored for any animation.
