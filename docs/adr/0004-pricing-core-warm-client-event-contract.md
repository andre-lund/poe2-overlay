---
status: accepted
---

# 0004 — Pricing core: warm async client, IP-rate-limit lockout, and a two-phase overlay event contract

**Implemented by:** [docs/plans/archive/0001-mvp-price-check-overlay.md](../plans/archive/0001-mvp-price-check-overlay.md) (T4), commits cd60a03 (T4)
**Builds on:** [ADR-0001](0001-clean-build-rust-tauri-layer-shell-overlay.md) (pricing data sources + persistent-app rationale) and [ADR-0002](0002-kde-global-shortcut-hotkey.md) (the hotkey path that drives a check)

The pricing core lives in the persistent app as a single warm `reqwest` async client plus
in-memory, daily-stale trade2 reference caches, held in Tauri state. A price check parses the
copied item text, then prices it: bulk/stackables via poe.ninja (no GGG quota) and gear/waystones
via the GGG trade2 `search`+`fetch` API. The hotkey worker thread drives the async pricing with
`tauri::async_runtime::block_on`. The overlay is fed by a **two-phase event contract**:
`price-check-loading` (item display name) shows the card immediately, then `price-check-result`
delivers a `PriceResult`. Every trade2 `search`/`fetch` is gated by an IP rate-limit lockout
derived from the `X-Rate-Limit-Ip` headers; poe.ninja bulk is not gated. The active league is
resolved from the live league list, never a hardcoded constant.

## Context

ADR-0001 chose the data sources (poe.ninja bulk, trade2 gear) and a persistent app so the HTTP
client/DNS stay warm between checks; the validated reference is PathofTrading's `backend.py`
(GPLv3 — technique reference only, reimplemented). T4 turns that into working code and must fix
two things the per-keypress Python reference left implicit:

- **The backend↔overlay interface.** T5 builds the listings UI, per-stat toggles, league selector,
  and requery on top of whatever T4 emits, so the event names and result shape are a cross-work-item
  contract that must be settled now, not an internal detail.
- **IP-ban safety.** GGG IP-bans abusive trade2 callers — the one unrecoverable failure mode
  (`docs/research/RESEARCH.md`). The reference self-throttles off the `X-Rate-Limit-Ip` headers;
  we must too.

Two reference details proved stale/dead against the live 2026 APIs (confirmed by smoke tests):
its hardcoded league is out of date, and its currency-name→poe.ninja-id map is defined but never
wired in (so common currency silently fell through to the GGG auction).

## Decision

- **Warm state.** One `reqwest` async `Client` + in-memory caches (trade2 `data/stats`,
  `data/items`, league list — 24 h TTL; poe.ninja exchange rates — 15 min TTL) live in a `Pricing`
  struct in Tauri state. Building the client needs no runtime; only sending does.
- **Async driven from the worker thread.** `price()` is `async`; the hotkey runs on a plain spawned
  `std::thread` (not a runtime worker) and blocks on it with `tauri::async_runtime::block_on`. T5's
  requery/league Tauri commands will be naturally async and share the same client. No std mutex
  guard is ever held across an `.await`.
- **Two-phase event contract.** `price-check-loading` carries the item display name (show the card);
  `price-check-result` carries a `PriceResult` serialized `camelCase`:
  `status` (`success` | `empty` | `rateLimited` | `error`), `item`, `message`, `listings`
  (`display`, `exaltVal`, `age`), `parsedStats`, `baseProperties`, `league`, `leagues`. `parsedStats`
  + `baseProperties` carry the toggle state forward so T5 can build per-stat toggles and requery
  without re-parsing.
- **IP-rate-limit lockout.** Both the `search` and `fetch` responses feed an `X-Rate-Limit-Ip`
  parser; when a window is within one request of its cap, a lockout is armed and every subsequent
  gear `search`/`fetch` is short-circuited to a `rateLimited` result until it clears. poe.ninja bulk
  is never gated (zero GGG quota).
- **League resolved live.** The queried league is the user override if it is a current league, else
  the first fetched economy league (the current challenge league); the hardcoded constant is only an
  offline fallback. GGG trade2 rejects a stale league in the search path, so this cannot be static.
- **Common currency via the currency-id map.** poe.ninja keys high-value orbs by short aliases
  (`divine`, not `divine-orb`) and returns null names; the name→id map (dead code in the reference)
  is wired in so common currency prices on the zero-quota poe.ninja path as ADR-0001 intended.

## Consequences

- T5 can build entirely against the `PriceResult` shape + the two event names; changing that shape
  later is a breaking change to that contract (record it as a new ADR if it moves).
- Pricing failures never panic the app — network/quota/parse failures become an
  `empty`/`error`/`rateLimited` result the overlay renders as text.
- The lockout is in-memory (per process); a fresh launch starts with a clear budget, which is safe
  because the warm process is long-lived and the headers re-arm it within one request.
- Trade2 reference data is fetched lazily on first check and re-fetched daily; a long-running overlay
  picks up a league/patch rollover within a day (and league rollover immediately, since the active
  league is resolved per check from the fetched list).
- Advanced item text (Ctrl+Alt+C affix tiers/ranges) remains a future one-line synth change; the
  parser already handles the annotated form when present.
