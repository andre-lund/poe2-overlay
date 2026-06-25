---
status: active
created: 2026-06-23
updated: 2026-06-25
adrs: [1, 2, 3, 4]
---

# Plan: MVP price-check overlay (then map/atlas + regex)

## Intent

Deliver a working PoE2 trade overlay on KDE Wayland: press a hotkey over an
in-game item and see live pricing in an overlay drawn on top of the game. This
is the floor that makes the project useful; the map/atlas danger-checker and
regex helpers build on the same overlay + input + data foundation afterward.

## Approach

Per [ADR-0001](../adr/0001-clean-build-rust-tauri-layer-shell-overlay.md): a Rust
+ Tauri 2 + Vue app whose GTK window is promoted to a `wlr-layer-shell` OVERLAY
surface; input via `evdev` (hotkey) + `uinput` (synthesized copy) + clipboard;
pricing via poe.ninja (bulk) and the GGG trade2 API (gear). The Tauri shell is
scaffolded; module seams exist in `src-tauri/src/{overlay,hotkey,trade}.rs`.
Reference implementations: ExileWatch (Rust/gtk-layer-shell), PathofTrading
(Quickshell, validated on this machine), Waystone (portal GlobalShortcuts) —
read, do not copy.

The make-or-break unknown is whether KWin composites the layer-shell OVERLAY
surface over *exclusive* fullscreen; T2 must test both fullscreen modes and
record the result (borderless is the known-good fallback).

## Tasks

- [x] **T1** — Build prereqs + confirm the empty Tauri shell runs. **Done:** all Tauri Linux build deps already present (webkit2gtk-4.1, gtk3, base-devel, openssl, librsvg, libappindicator-gtk3) — no sudo install needed; `npm install` + `npm run build` pass; `cargo build` clean (4 expected stub warnings); `npm run tauri:dev` launches the hidden window (vite :1420, `Running target/debug/poe2-overlay`, no panic — only a benign `libx265` GStreamer plugin-scan warning). Added `tauri:dev`/`tauri:build` scripts setting `GDK_BACKEND=wayland` + `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
- [x] **T2** — Layer-shell overlay surface: add `gtk-layer-shell`/`gdk`, promote the main window (Layer::Overlay, anchors, on-demand keyboard), show/hide. **Verify over a real Proton PoE2 game.** **Impl:** activated `gtk = "0.18"` + `gtk-layer-shell = { "0.8", features = ["v0_6"] }` (matches Tauri 2.11's gtk-rs 0.18; `v0_6` gates `KeyboardMode`). `overlay::init_layer_shell` promotes the still-hidden GTK window — `Layer::Overlay`, **all-four-edge anchors (full-output surface)**, `exclusive_zone(-1)`, `KeyboardMode::OnDemand`, namespace `poe2-overlay`. The card is positioned top-right via CSS within the full-screen canvas; `hide_overlay` command unmaps it; ✕ button + Esc dismiss. **Make-or-break CONFIRMED:** the full-output overlay composites over fullscreen Proton PoE2 — the user saw the popup drawn on top of the running game. **Two corrected dead-ends:** (1) full-output + `set_ignore_cursor_events` click-through trapped *all* input — tao sets the input-shape on the *toplevel* GDK window, which the WebKitGTK child surface ignores, so the transparent screen ate clicks with no exit (user-reported); fix = a ✕/Esc-dismissable modal (PathofTrading's full-screen, focusable, show-on-demand model) rather than click-through. (2) a corner-sized surface (top+right, 2 anchors) collapses to ~0 size — gtk-layer-shell takes the size from the WebKitGTK child whose min-size request is ~0, so nothing renders (user saw nothing); **full-output (4 anchors) is required** to force a non-zero size. (`Exclusive` keyboard focus also rejected — it drops the game out of fullscreen.) Compiles clean (only T3/T4 stub warnings); surface maps (instrumented `map-event` + `size-allocate`). **Tooling caveat:** `spectacle`/KWin screencast does not reliably capture the transparent overlay layer over a fullscreen game on this HDR setup (captured it once on a clean desktop, not since) — visual confirmation is by eye, not screenshot. **Done (user-confirmed):** the card shows top-right composited over fullscreen PoE2 and ✕/Esc dismisses. While shown it is modal (covers the screen); T3 hides it by default + shows on the hotkey.
- [x] **T3** — Input path: global hotkey → `uinput` Ctrl+C synth → X11 clipboard read. **Pivoted to a KDE global shortcut (ADR-0002), not evdev:** evdev can read keys but not *consume* them, so a chord on a game-bound key (PoE2's `D`) leaks to the game and moves the character (confirmed: Alt+D moved the character). A KWin global shortcut intercepts the chord first — the proven PathofTrading mechanism. **Impl (code-complete; in-game test pending):** `Ctrl+Alt+D` is a KDE service shortcut running `poe2-overlay --price-check`; `tauri-plugin-single-instance` (first plugin) forwards it to the running app → `hotkey::price_check` synthesizes Ctrl+C via a warm `uinput` device (`Synth` in Tauri state) → 120 ms → reads the X11 CLIPBOARD selection (`x11-clipboard` crate, no `xclip` binary) → emits `price-check-item` + shows the overlay; ✕/Esc hides. No `/dev/input` reads, so **no `input` group / sudo** — only the `/dev/uinput` session ACL. Verified end-to-end minus the game: second-instance forwarding fires `price_check`, synth device builds, clipboard read returns text (27 chars seeded). Took over PathofTrading's `Ctrl+Alt+D` (kglobalshortcutsrc + a `poe2-overlay-pricecheck.desktop`; backup saved). **Input-trap incident + fix ([ADR-0003]):** the first in-game trigger locked the machine out — `window.show()` mapped the **full-output** T2 surface, whose `wl_surface` input region (CSS `pointer-events:none` does not shrink it) swallowed every click with only an on-screen ✕/Esc to escape; hard-restart required. Fixed by making the surface a **fixed-size sub-output rectangle** (`set_size_request` **+ `resize(1,1)`** — tao pre-pins the window to the conf size 1140×600, so the `resize` is what commits the real size; corrects T2's "full-output required", which was just a missing size request) so it can never cover the screen, plus a compositor-level **`Ctrl+Alt+X` → `--hide`** shortcut (single-instance forwarding, ADR-0002) as a guaranteed escape. Adversarially audited — no residual lockout. **Then three functional bugs, fixed in order:** (1) **repeat checks read an empty clipboard** — *not* the focus theory first chased (KeyboardMode/GTK focus props made no difference); the real cause is an **XWayland clipboard read-race** — the game's copy reaches the X11 selection only after KWin's sync, and a single read at 120 ms catches a transient-empty mid-sync state. Fixed by **polling** the clipboard (~40 ms × up to 20, ~800 ms) until non-empty, matching PathofTrading's retrying backend. (2) **stacked ghost popups** — a content-sized card shrinks for shorter items and WebKitGTK leaves the previously-painted transparent region uncleared until a later repaint. Fixed with a **fixed-size 400×380 card** that overpaints the same region every time, plus a `show()`-only-when-hidden guard. (3) **panel moved to screen-centre** (per user; the corner was too far to glance at) — surface unanchored+centred 440×420. `KeyboardMode::None` kept on its own merit (a game overlay must not steal the keyboard from PoE2). **Done (user-confirmed in-game):** hover item + Ctrl+Alt+D shows the centred price card; repeat checks replace it cleanly; ✕/Ctrl+Alt+X dismiss; no lockout. **Follow-ups (T6):** `Ctrl+Alt+X` needs the live `kglobalacceld` to reload (relogin) to fire; and a first launch with no running instance drops its own `--price-check`/`--hide` flag (the single-instance callback fires only for the 2nd instance) and leaves a stray hidden process holding `/dev/uinput` — fold into autostart.
- [x] **T4** — Pricing core: parse PoE2 item text; bulk via poe.ninja, gear via GGG trade2 search+fetch with `X-Rate-Limit` handling; persistent warm HTTP client. **Done (code-complete; in-game test pending):** New `trade/` module — a faithful Rust reimplementation of PathofTrading's `backend.py` (GPLv3, technique reference only, [ADR-0004]): `parse.rs` (item-text parser, bulk-vs-gear, stats w/ affix tier+source), `stats.rs` (`StatMapper` fuzzy stat-text→trade2 id w/ pseudo totals + reduced→increased / less→more / synonym / chance-to fallbacks; `base_name` base-type resolution), `ninja.rs` (poe.ninja bulk + exchange rates), `gear.rs` (trade2 query build → search+fetch → exalt-normalized sorted listings), `mod.rs` (warm async `reqwest` client + daily-stale trade2 `data/stats`/`data/items`/league caches + 15-min exchange-rate cache, held in Tauri state; IP `X-Rate-Limit` lockout; live league resolution). Async pricing is driven from the hotkey worker thread via `tauri::async_runtime::block_on` (the thread is a plain `std::thread`, not a runtime worker, so this is safe). `hotkey::price_check` now parses → emits `price-check-loading` → prices → emits `price-check-result` (two-phase contract); `App.vue` renders it minimally (rich per-stat-toggle / league-selector / requery UI is T5). **Two stale-reference bugs caught by live API probing + fixed:** (1) the reference's hardcoded league "Fate of the Vaal" is dead — the current league is "Runes of Aldur", and a stale league in the trade2 search path returns HTTP 400 "Invalid query"; we now resolve the active league from the live league list (poe.ninja is lenient about the league param, which masked this on the bulk path). (2) the reference's `NINJA_CURRENCY_MAP` is dead code — poe.ninja keys high-value orbs by short ids (`divine`, not `divine-orb`) and returns null names, so common currency silently fell through to the GGG auction; we wired the map in so currency prices on the zero-quota poe.ninja path. **Verified end-to-end against live APIs** (ignored smoke tests, `cargo test -- --ignored`): bulk Divine→"1 D", Chaos→"38.51 E"; a rare body-armour gear search → Success, 10 listings, pseudo total-life + pseudo total-elemental-resistance filters correctly mapped (75% res ×0.8 = min 60; 89 life ×0.8 = min 71). `cargo build` + `clippy` clean; 11 offline unit tests pass. **Adversarial multi-agent review (5 dimensions, each finding independently verified) → 4 confirmed, all fixed:** (high) a 429 never armed GGG's `Retry-After` penalty into the lockout — only the ~1 s `window/limit` estimate — so a mashed hotkey could re-fire into an active penalty and escalate toward an IP ban → now arm the lockout from `Retry-After` on every 429, also read the active-restriction header field, and added an `IN_FLIGHT` guard so re-entrant triggers are dropped; (med) Normal-rarity base type (the "Superior " quality prefix) wasn't resolved via the item-type list → widened the deferred resolution to all non-Unique/Rare items; (med) std-mutex poisoning + a `from_secs_f64` overflow on a hostile rate-limit header could panic/brick pricing → poison-recovering locks + finite-checked, hour-clamped, `checked_add` lockout arming; (low) the truncated-Rare fallback now matches the reference's `lines[0]`. **Follow-up:** confirm in-game that a hovered rare/currency shows live prices, and that the parser's `Level:`→gem-level rule does not misfire on real PoE2 gear requirement lines (assumed "Requires: Level N", which is metadata-skipped — verify).
- [x] **T5** — Overlay UI (Vue): listings, per-stat filter toggles, requery, league selector; transparent/click-through styling. **Done (code-complete; in-game test pending):** Implements the ADR-0004 contract end-to-end. **Backend:** extracted `gear::run_gear_query` from `price_gear` (runs the trade2 search+fetch from *pre-built* filters, so requery reuses it with user-edited ones); `Pricing` gained `last_item` (the last `ParsedItem`, so requery re-prices without the frontend round-tripping the whole item), `set_league(Option<String>)` override, and `requery(league, parsed_stats, base_properties)` (bulk re-priced via poe.ninja for the new league, gear via `run_gear_query` with the edited filters, IP-lockout-gated); `ParsedStat`/`BaseProp` gained `Deserialize` to round-trip from JS; the rates cache now keys on `rates_league` so a league switch refetches+replaces (was a single unkeyed map → stale on switch). An async `#[tauri::command] requery` (in `lib.rs`) bridges the overlay to it. **Frontend (`App.vue`):** league `<select>` (requeries on change), toggleable base-property + stat-filter rows (checkbox + editable min/max), an explicit **Requery** button (not auto-requery-per-keystroke — that would risk the GGG IP limit), loading-vs-busy states, polished listings. Surface enlarged to 470×640 (`overlay.rs`) for the richer panel; the card stays a constant-size internally-scrolling region (the T3/ADR-0003 ghost-repaint fix). **Verified:** `cargo build` + `clippy` clean; 11 offline unit tests; frontend `vue-tsc` type-check; **3 live smoke tests pass** (bulk Divine→"1 D"/Chaos→"38.82 E"; rare gear search→Success/10 listings; **requery with edited filters→Success/10**). **Adversarial multi-agent review (4 dimensions, each verified) → 4 confirmed, all fixed:** (high) the requery lockout pre-check emptied `parsed_stats`/`base_properties`, which `applyResult` blindly repopulates from → wiped the user's filter edits; now threads the edited filters back (matching the reference); (med) a failed exchange-rate refetch on league switch silently served the *previous* league's rates as the new league's prices → falls back to neutral seeds + forces a retry; (med) filter checkboxes/min-max stayed editable during an in-flight requery so edits got clobbered on resolve → `:disabled="busy"`; (low) a hotkey check fired mid-requery could be overwritten by the stale requery result → a `reqGen` generation token drops the stale result. **Follow-up:** in-game visual pass (toggles + league selector + requery against a real hovered item; the transparent layer-shell surface can't be screenshotted headless).
- [ ] **T6** — Package + launch: AppImage build, KDE global shortcut + `ydotoold`-equivalent / uinput setup docs, autostart.
- [ ] **T7** — (post-MVP) Map/atlas danger-checker: flag dangerous waystone/map mod combinations.
- [ ] **T8** — (post-MVP) Regex helpers: stash/vendor search regex builder + cheat-sheets.

## Decision log

- Scope split: T1-T6 = price-check MVP; T7-T8 (map/atlas + regex) are in project
  scope but sequenced after the MVP works end-to-end.
- gtk-layer-shell/evdev/reqwest are declared (commented) in `Cargo.toml` and
  activated per task to keep the scaffold buildable before system libs are present.
- T2 overlay model: corner-sized surface (top+right, 460×160), hidden-by-default,
  shown on demand, dismissable — matching the proven PathofTrading reference. NOT a
  full-output click-through canvas: `set_ignore_cursor_events` sets the input-shape on
  the toplevel GDK window, which the WebKitGTK child surface ignores, so a full-output
  surface eats every click. Confirmed compositing over fullscreen PoE2 (user saw the
  corner popup over the running game), so the make-or-break is settled for this stack.
- T2 fallback (if a future fullscreen mode ever hides it): borderless/Windowed
  Fullscreen is the PathofTrading-validated config; exclusive fullscreen could in
  principle trigger a KWin direct-scanout bypass. Diagnostic lever if so:
  `KWIN_DRM_NO_DIRECT_SCANOUT=1` in the game env, or System Settings → Display &
  Monitor → Compositor "Allow applications to block compositing".
- T2 tooling note: `spectacle`/KWin screencast does not reliably capture a transparent
  layer-shell surface composited over fullscreen (returns a transparent frame). Verify
  the overlay by eye, not by screenshot.
- T5 will decide the real panel: a larger sized surface still has transparent dead
  zones that catch clicks within its bounds; per-region input (or content-sized
  surface) is the T5 seam — not needed for the corner probe.
- T3 input mechanism pivoted evdev → KDE global shortcut ([ADR-0002]). evdev can't
  consume keys, so a game-bound chord (PoE2 `D`) leaks and moves the character. The
  KDE shortcut consumes the chord and removes the `input`-group requirement. `xclip`
  (ADR-0001's pick) isn't installed; we read the X11 selection in-process via the
  `x11-clipboard` crate — no external binary, same X11 selection.
- T3 copy is plain Ctrl+C (PoE2 basic copy, confirmed by PathofTrading's
  run_pricecheck.sh). Advanced item text (Ctrl+Alt+C) is a one-line synth change
  once basic pricing works.
- **Overlay surface model: full-output → sized, centred, focus-free ([ADR-0003],
  supersedes the T2 full-output decision above).** The full-output surface trapped all
  screen input (CSS `pointer-events:none` does not shrink a `wl_surface` input region)
  and locked the user out → hard restart. The surface is now a fixed-size unanchored
  (centred) rectangle, sized with the gtk-layer-shell two-call idiom
  (`set_size_request(440,420)` **then** `resize(1,1)` — tao pre-pins the window to the
  conf size 1140×600 and `set_size_request` only raises the minimum, so the `resize`
  is what commits the real size; T2's "collapse-to-0" was simply a missing size
  request). It cannot cover the screen. Plus **`KeyboardMode::None`** (a game overlay
  must not steal the keyboard from PoE2) and a compositor-level `Ctrl+Alt+X` → `--hide`
  escape. Per-region click-through *within* the panel stays the T5 seam.
- **Repeat-checks empty clipboard = XWayland read-race, NOT focus (dead-end recorded so
  it is not re-chased).** First theory was focus theft (overlay grabs focus → clipboard
  bridge empties); `KeyboardMode::None`, hide-before-copy, and GTK `accept_focus`/
  `focus_on_map(false)` ALL failed to change the symptom. Real cause: the game's copy
  reaches the X11 CLIPBOARD only after KWin's XWayland sync, and a single read at 120 ms
  catches a transient-empty state (proven by the diagnostic that the item appeared on
  the *next* press's pre-read). Fix: **poll** the clipboard (~40 ms × up to 20) until
  non-empty — the same retry PathofTrading's backend does. The speculative focus props
  were removed in the cleanup; `KeyboardMode::None` stays for the keyboard reason above.
- **Stacked ghost popups = WebKit transparent-repaint, fixed by a constant-size card.**
  A content-sized card shrinks for shorter items; WebKitGTK does not clear the
  previously-painted transparent region until a later repaint, so old cards linger
  stacked (they clear "after some time"). Fix: a **fixed 400×380 card** that overpaints
  the same region each update, plus `show()` only when the window is hidden (calling it
  on an already-mapped layer surface was a suspected second cause; the fixed size was
  the actual fix).

- **T4 pricing architecture is [ADR-0004]:** warm async `reqwest` client + daily-stale
  trade2 reference caches in Tauri state; async pricing driven from the hotkey worker
  thread via `block_on`; a two-phase `price-check-loading` → `price-check-result` event
  contract (the `PriceResult` shape T5 builds its toggles/requery on); IP rate-limit
  lockout gating every trade2 search/fetch (poe.ninja bulk ungated). Recorded as an ADR
  because the event contract + IP-ban policy outlive T4.
- **The `backend.py` reference has two stale/dead spots we deliberately diverge from
  (verified against the live 2026 APIs):** its hardcoded league is out of date, so we
  resolve the active league from the fetched league list (GGG trade2 rejects a stale
  league path → 400); and its `NINJA_CURRENCY_MAP` is defined but never wired in, so we
  use it (poe.ninja keys orbs by short ids + returns null names) to keep common currency
  on the zero-quota poe.ninja path per ADR-0001.
- **poe.ninja denomination (confirmed empirically):** the Currency exchange overview is
  divine-based (`divine`=1), normalized to exalt-equivalents in `fetch_exchange_rates`;
  the per-category overviews (Essences/Runes/Omens/…) are already exalt-denominated, so
  their `primaryValue` is used directly — the reference's `÷ rates["exalt"]` was a no-op,
  removed for clarity, not a behavior change.
- **Stat-filter fidelity is faithful to the reference:** most stats map to pseudo
  aggregates (total life/mana/ES/res/attributes), single + all elemental resistances are
  summed into one pseudo total-elemental-resistance filter, and the seed bound is 80% of
  the rolled value (full value for Grants Skill / Bonded / Adds; negative rolls bound the
  `max`). Pseudo ids confirmed present in PoE2's `data/stats`.
- **Rate-limit lockout hardened past the reference (review finding):** the reference (and
  our first cut) only self-throttled `window/limit` (~1 s) on a 429 and never armed GGG's
  real `Retry-After` penalty — fine per-keypress where the human throttles, dangerous in a
  persistent app a hotkey can hammer. We arm the lockout from `Retry-After` on every 429,
  read the active-restriction header field, clamp to 1 h, never shorten a live lockout, and
  drop re-entrant checks with an `IN_FLIGHT` guard. This is the IP-ban path ADR-0004 exists
  to prevent.
- **T4 verifies by ignored network smoke tests, not in-game:** `cargo test -- --ignored`
  hits poe.ninja (bulk) and one trade2 search+fetch (gear) — enough to confirm the live API
  shapes + the full pipeline without the game. In-game confirmation (a real hovered item) is
  the T4 follow-up folded into T5/T6 testing.

- **T5 requery keeps item identity server-side.** Rather than serialize the whole item
  to JS and back, `Pricing` stores the last `ParsedItem` and the requery command carries
  only the edited filters + league; the backend re-prices the stored item. Matches the
  reference's `--requery-data` intent without the round-trip. `run_gear_query` is the
  shared core so initial-check and requery build the identical query.
- **Requery is an explicit button, not auto-per-keystroke; league change requeries
  directly.** Auto-requerying on every toggle/min-max edit would fire a GGG search per
  keystroke and risk the IP limit (ADR-0004). Toggles stage locally; one button (and a
  league change, a single deliberate action) fires the search. Controls are disabled
  while a requery is in flight.
- **Exchange rates are keyed by league (`rates_league`).** T4 cached a single unkeyed
  rate map; with the T5 league selector that served stale cross-league prices on a
  switch. Rates now refetch+replace when the league changes, and a *failed* refetch for a
  new league drops to neutral seeds (never the prior league's ratios) and retries next
  check.
- **T5 surface is 470×640, card fills it with internal scroll.** Settles the "T5 will
  decide the real panel" seam: a constant-size card (the T3/ADR-0003 ghost-repaint fix)
  large enough for the league selector + filter toggles + listings, bounded so clicks
  outside still reach the game.
- **No new ADR for T5** — it implements the ADR-0004 backend↔overlay contract; the local
  build decisions live here.

[ADR-0002]: ../adr/0002-kde-global-shortcut-hotkey.md
[ADR-0003]: ../adr/0003-overlay-dismissal-safety-corner-surface-hide-shortcut.md
[ADR-0004]: ../adr/0004-pricing-core-warm-client-event-contract.md
