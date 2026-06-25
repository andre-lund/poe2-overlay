---
status: accepted
---

# 0006 — Regex helpers ship as a pointer-only cheat-sheet that writes the X11 clipboard

**Implemented by:** [docs/plans/active/0001-mvp-price-check-overlay.md](../plans/active/0001-mvp-price-check-overlay.md) (T8)
**Builds on:** [ADR-0002](0002-kde-global-shortcut-hotkey.md) (KDE-shortcut trigger), [ADR-0003](0003-overlay-dismissal-safety-corner-surface-hide-shortcut.md) (the never-take-keyboard-focus surface), and [ADR-0004](0004-pricing-core-warm-client-event-contract.md) (the overlay event-contract pattern)

The regex-helpers feature is a **curated, pointer-only cheat-sheet**: a static library of PoE2
stash/vendor search-regex patterns the user clicks to copy, then pastes (`Ctrl+V`) into the
game's `Ctrl-F` box. It is opened by its own KDE global shortcut (`Ctrl+Alt+F` → `--regex`,
same single-instance mechanism as price-check/hide), shows a new cheat-sheet panel in the
overlay (`show-regex` event), and copies a pattern by **writing the X11 CLIPBOARD selection**
(`copy_to_clipboard` command). No interactive/typed builder, no auto-paste.

## Context

PoE2's in-game stash and vendor search support a regex-like syntax (alternation, character
classes/ranges, `.`, quantifiers, anchors, the `!` exclusion, space-ANDed blocks) matching the
item's full text including its mods — confirmed for PoE2 specifically (not just PoE1) by the GGG
forum + community sources (research, 2026-06-25). So the useful artifact is a search *string*;
the game does the matching. The hard part is the correct, PoE2-grounded patterns — authored
data, exactly like the `danger` ruleset.

Two constraints shape the design:
- **The overlay never takes keyboard focus** (ADR-0003, `KeyboardMode::None`). A typed regex
  editor is therefore structurally impossible — every interaction must be a mouse click.
- **The transport into the game is the clipboard.** The app already *reads* the X11 selection
  (the Proton/XWayland game writes there; ADR-0001/0002); the cheat-sheet must *write* it.

The user chose (over a build-aware or interactive-builder alternative) the cheat-sheet scope and
the fold-into-its-own-shortcut trigger.

## Decision

- **Pointer-only cheat-sheet, not a builder.** A categorized, click-to-copy list rendered from a
  static Rust const pattern table (`cheatsheet`), surfaced via the `get_cheatsheet` command. The
  no-typing constraint makes this the natural shape; an interactive composer (and free-text
  editing) is deferred to v2 only if real use demands it. A dedicated external generator already
  exists for power composition.
- **Own its own KDE shortcut.** `Ctrl+Alt+F` → `poe2-overlay --regex` → the single-instance
  handler emits `show-regex` and shows the overlay in cheat-sheet mode. The panel is not
  item-driven, so it is *not* folded onto the price-check key (which carries item context). The
  installer adds the third `.desktop` + `kglobalshortcutsrc` binding (T6).
- **Copy via an X11 CLIPBOARD write, owned by the warm instance.** A persistent
  `x11_clipboard::Clipboard` is held in Tauri state; `copy_to_clipboard` calls its non-blocking
  `store`, and its background thread serves the game's paste for the process lifetime. Both the
  app's owner and the game are XWayland X11 clients on the same server, so selection serving is
  native — the write-side mirror of the existing read. This is the app's first clipboard *write*.
- **Clipboard-only, no auto-paste.** The app does not synthesize `Ctrl+V` — it cannot verify the
  in-game search box is focused, so auto-paste risks dumping the regex into chat/inventory and
  re-opens the input-synthesis timing fragility the copy path already fights. The single manual
  paste is cheap and safe.
- **Search length is one named constant** (`SEARCH_CHAR_LIMIT`, default 250 — version-ambiguous
  across Early Access patches) surfaced to the UI; patterns are kept short and the const is the
  single place to adjust.

## Consequences

- The overlay now serves three panels over its hotkeys — price (Ctrl+Alt+D / non-waystone),
  danger (Ctrl+Alt+D / waystone, ADR-0005), and the regex cheat-sheet (Ctrl+Alt+F); the frontend
  must keep them mutually exclusive and swap cleanly.
- The pattern table is living, patch-sensitive content (mod phrasings + the `rarity:`/tier syntax
  drift across EA patches) — isolated as data with `verify` notes, refined like the danger ruleset;
  correctness on the user's patch is confirmed by an in-game smoke test.
- A failed X11 clipboard open disables only the copy (logged); the rest of the overlay is
  unaffected. The under-Proton paste is verified in-game (the ownership/atoms model is sound but
  the live behavior is unproven here).
- v2 space (only if used): a pointer-only composer, per-user favorites, auto-paste via the
  existing uinput synth — any of which would supersede the relevant part of this decision.
