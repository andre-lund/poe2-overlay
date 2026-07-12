---
status: superseded by ADR-0007
---

# 0003 — Overlay dismissal safety: a sized, focus-free surface + a compositor-level hide shortcut

**Implemented by:** [docs/plans/active/0001-mvp-price-check-overlay.md](../plans/active/0001-mvp-price-check-overlay.md) (T3), commits _pending_
**Relates to:** ADR-0002 (reuses its single-instance shortcut-forwarding mechanism for `--hide`)

The overlay surface must **never** be the sole owner of its own dismissal, and it
must **never** steal focus from the game. Three guarantees enforce this:

1. The layer-shell surface is a **fixed size, centered** on the output
   (`set_size_request` + the `resize(1,1)` idiom), never stretched to the whole
   output — so it occupies only a bounded patch and clicks elsewhere reach the game.
2. The surface uses **`KeyboardMode::None`** — it never takes keyboard focus, so it
   can never become a focus-grab that the user must escape, and the game keeps the
   keyboard (you keep moving/casting while the overlay is up).
3. A **KDE global shortcut** (`Ctrl+Alt+X`) forwards `--hide` to the running app
   (same single-instance path as the price-check trigger, ADR-0002), which calls
   `window.hide()`.

## Context

T2 ran the overlay as a **full-output** layer-shell surface (anchored to all four
edges) on `Layer::Overlay`, intending the web content's CSS `pointer-events: none`
to let clicks fall through everywhere except the price card. On Wayland that is
false: a `wl_surface`'s input region defaults to the entire surface, and CSS
`pointer-events` only affects DOM hit-testing *inside* the webview — it does not
shrink the surface's input region. So a full-output surface swallows **every**
pointer event on the screen; nothing reaches the game or desktop. With keyboard set
to `OnDemand` (focus only after a click) and the only dismiss affordances being an
on-screen ✕ and Esc, a shown overlay whose card failed to render — or whose ✕ the
user could not hit — trapped all input with no escape. This happened: the machine
had to be hard-restarted.

T2 had chosen full-output because a corner surface "collapsed to ~0" — gtk-layer-shell
sizes an *unstretched* surface from its child (the WebKitGTK webview, whose minimum
size request is ~0). The missing step was forcing the size; T2 never called
`set_size_request`.

`KeyboardMode::Exclusive` was rejected because it drops the game out of fullscreen,
and any focus-taking mode steals the keyboard from PoE2, which a game overlay must not
do. (The empty-clipboard-on-repeat-checks symptom that first looked like a focus
problem turned out to be an unrelated XWayland clipboard read-race — recorded in the
plan, not here; this ADR is only about the overlay surface's safety properties.)

## Decision

- **Sized, centered surface.** Anchor no opposite-edge pair (with all edges
  unanchored gtk-layer-shell centers the surface; an unstretched surface keeps its
  window size, never the whole output). Force the size with gtk-layer-shell's two-call
  "Forcing Window Size" idiom — `set_size_request(440, 420)` **then** `resize(1, 1)`:
  tao pre-pins the window to the `tauri.conf.json` size (1140×600), and on a resizable
  toplevel `set_size_request` only raises the *minimum*, so `set_size_request` alone
  maps the surface ~1140 wide; the `resize(1, 1)` (clamped back up to the minimum)
  clears tao's sticky size and commits the true 440×420 surface. The surface is then a
  finite, centered rectangle; clicks anywhere outside it reach the game/desktop. If
  size handling ever failed the surface is ~1×1 — invisible and trapping nothing —
  **never** full-screen. This makes a whole-screen input trap structurally impossible;
  it is the *primary* backstop. The hide shortcut below is the *secondary* backstop and
  depends on the GTK main loop still being live.
- **Never take focus.** `KeyboardMode::None`: the surface receives pointer events (so
  the ✕ still works) but never keyboard focus, so the game keeps the keyboard while the
  overlay is up. In-webview Esc is therefore inert.
- **Compositor-level escape.** A KDE global shortcut (`Ctrl+Alt+X`) launches
  `poe2-overlay --hide`; `tauri-plugin-single-instance` forwards it to the running
  instance, which hides the window. Because KWin owns the chord, it reaches the app
  even if the surface were grabbing all input — so dismissal never depends on the
  surface rendering, on the webview being responsive, or on the surface holding
  focus.
- The ✕ button (a focus-independent pointer click) is the in-panel dismiss; with
  `None`, Esc no longer fires and is not relied on.
- Per-region click-through *within* the panel (so the game stays live behind a
  visible card) remains a T5 concern; it is orthogonal to this safety invariant.

## Consequences

- The overlay can no longer lock the user out of the desktop. The sized surface, the
  focus-free keyboard mode, and the hide shortcut are independent — each alone prevents
  the lockout; together they are belt-and-suspenders.
- The price-check trigger (`Ctrl+Alt+D`, ADR-0002) is unchanged and may re-run while
  the overlay is visible (good for checking items in sequence); dismissal is a
  separate key, not a toggle on the trigger.
- The hide shortcut needs a second KDE binding (`poe2-overlay-hide.desktop` +
  `kglobalshortcutsrc`), set up by the installer/docs (T6) alongside the price-check
  binding. The sized surface and the keyboard mode fix nothing the installer must do —
  they are pure code.
- Within the centered rectangle, transparent areas around the card still consume
  clicks; this is a small, bounded dead-zone over the screen centre (not the whole
  screen) and is the T5 per-region-input seam, as already noted in the plan. Centre
  placement was the user's choice (a corner was too far to glance at); it trades a
  more central dead-zone while shown for readability.
