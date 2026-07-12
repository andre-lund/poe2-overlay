---
status: accepted
---

# 0007 — On-demand keyboard focus so typed filters work, keeping the dismissal-safety posture

**Implemented by:** [docs/plans/archive/0001-mvp-price-check-overlay.md](../plans/archive/0001-mvp-price-check-overlay.md) (T11), commits eb8c8c6 (T11) · c2b0ff1 (T11)
**Supersedes:** ADR-0003
**Relates to:** ADR-0002 (the `--hide` forwarding path it keeps relying on)

The overlay surface uses **`KeyboardMode::OnDemand`**: it takes keyboard focus only
after a click on the panel, and the game keeps the keyboard at all other times. The
other two ADR-0003 guarantees — the fixed-size centered surface and the
compositor-level `Ctrl+Alt+X` hide shortcut — are carried forward unchanged.

## Context

ADR-0003 chose `KeyboardMode::None` when the overlay was a pointer-only UI (✕ button,
checkboxes). T5 and T9 then added typed inputs — the stat min/max bounds and the
price-sheet name filter — without revisiting that choice; with `None` the surface
never receives keyboard focus, so those fields can never accept a keystroke in-game
and are mouse-decoration only. The original lockout incident that motivated ADR-0003
implicated `OnDemand` only in combination with the **full-output** surface (a
whole-screen input trap whose sole escape needed focus-dependent affordances); the
sized sub-output surface has since made that trap structurally impossible on its own.

## Decision

- **`KeyboardMode::OnDemand`** on the layer-shell surface. Idle behavior is identical
  to `None` — the game keeps the keyboard while the overlay is up; focus moves to the
  panel only when the user deliberately clicks it (to type into min/max or the sheet
  filter), and returns to the game when they click back into it.
- **Carried forward from ADR-0003, unchanged:** the fixed-size centered surface
  (`set_size_request` + `resize(1,1)`; a whole-screen input trap stays structurally
  impossible) and the KWin-owned `Ctrl+Alt+X → --hide` escape, which works regardless
  of focus, rendering, or webview responsiveness. `Exclusive` remains rejected (drops
  the game out of fullscreen).
- In-webview `Esc` becomes a live dismiss whenever the panel holds focus (it was inert
  under `None`); the ✕ click and `Ctrl+Alt+X` remain the focus-independent paths.

## Consequences

- The stat min/max fields (T5) and the sheet name filter (T9) become functional
  in-game: click the field, type, click back into the game.
- While the panel holds focus the game does not see the keyboard — the deliberate
  trade the user makes by clicking a text field; movement/casting resumes on clicking
  the game.
- **Verification gate (in-game):** confirm that clicking the overlay under a
  fullscreen Proton PoE2 does not knock the game out of fullscreen or trigger a
  minimize-on-focus-loss, and that focus returns cleanly on a game click. If it
  misbehaves, the fallback is reverting to `None` (a one-line change) and accepting
  mouse-only filters — tracked in the plan (T11).
- The dismissal-safety analysis of ADR-0003 otherwise stands: the sized surface and
  the compositor shortcut are each independently sufficient against a lockout.
