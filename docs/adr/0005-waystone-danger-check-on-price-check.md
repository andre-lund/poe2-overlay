---
status: accepted
---

# 0005 — A price-check on a Waystone yields a danger verdict, not a price (separate event, static keyword ruleset)

**Implemented by:** [docs/plans/archive/0001-mvp-price-check-overlay.md](../plans/archive/0001-mvp-price-check-overlay.md) (T7), commits 0b1fcf1 (T7)
**Builds on:** [ADR-0002](0002-kde-global-shortcut-hotkey.md) (the hotkey/clipboard/parse path) and [ADR-0004](0004-pricing-core-warm-client-event-contract.md) (the overlay event contract this extends)

The price-check hotkey is overloaded: when the copied item is a **Waystone** (item class
`Waystones`), the app does **not** price it. Instead it analyzes the waystone's mods
against a curated, build-agnostic danger ruleset and emits a `price-check-danger` event
carrying a `DangerReport` (a `DangerLevel` — Safe / Caution / Dangerous / Deadly — plus
the matched dangers); the overlay shows a danger panel. The analysis is local, synchronous
and uses no GGG quota. Non-waystone items price exactly as before.

## Context

PoE Overlay II's map/atlas danger-checker is a wanted feature (`docs/research/RESEARCH.md`),
but there is no reusable reference for it — the ruleset is opinionated PoE2 domain knowledge.
For a waystone, the player's question before running it is "is this safe?", not "what's it
worth": waystones are cheap and the danger verdict is the value, so pricing one (a GGG trade2
round-trip + rate-limit budget) is wasted work. The parser already extracts a waystone's mods
into its stat list (waystones route through the gear path), so the mods are in hand for free.

A grounding pass against the live trade2 `data/stats` (2026-06-25) established that **PoE2
waystones do not carry the PoE1-staple danger mods** (no damage reflection, no "cannot
regenerate", no "-% maximum resistances", no "extra damage as element" in the PoE1 phrasing).
The real lethal surface is curses, monster crit/damage/projectiles/speed, "Monsters deal % of
Damage as Extra <Element>" (Chaos worst — it bypasses Energy Shield), reduced recovery, Mark
for Death, Delirium, and assorted status effects.

The trigger and ruleset scope were chosen with the user: fold into the existing price-check
hotkey (no second shortcut), and a static curated ruleset (build-awareness deferred).

## Decision

- **Overload the price-check hotkey by item type.** `hotkey::price_check`, after parsing,
  branches: a waystone goes to the danger-checker and returns; everything else prices.
  No new KDE shortcut.
- **Waystones are not priced.** The danger verdict replaces the price for them. (Re-adding
  optional waystone pricing later is a small, additive change.)
- **Separate event + module, decoupled from pricing.** A new top-level `danger` module owns
  `DangerReport` / `DangerLevel` / `DangerFlag` and `analyze(&ParsedItem)`; the verdict is
  emitted on its own `price-check-danger` event (not folded into `PriceResult`). The overlay
  renders a danger panel or the price card depending on which event fired.
- **Static, build-agnostic keyword ruleset.** Each rule is a set of lowercased substring
  keywords (robust to the rolled numbers in the copied text) → a severity + label + "why",
  grounded in the real PoE2 mod phrasings. A few combinations (monster crit-chance + crit-
  damage; reduced recovery + amplified damage) escalate to `Deadly`. The report level is the
  max severity of its matched flags. The ruleset is plain data, refined over time.

## Consequences

- Checking many waystones in a row is instant and costs no GGG quota — the danger path never
  touches the network or the rate-limit lockout.
- The ruleset is the feature's quality surface and will drift as PoE2 patches add/rename map
  mods; it is isolated as data so refinement is low-risk. Severities are deliberately simple
  and opinionated (build-agnostic) — a future build-aware mode (the rejected T7 option) would
  tailor them, and would supersede this if adopted.
- Tablets and other map devices are out of scope (waystones only) for now.
- The overlay now has two result shapes over one hotkey; the frontend must swap panels cleanly
  between price and danger checks (handled in `App.vue`).
