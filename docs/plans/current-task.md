plan: active/0001-mvp-price-check-overlay
active: in-game verification pass. T1–T11 code-complete (MVP price-check + waystone danger-checker + regex cheat-sheet [dormant] + category price sheet + PoE2 retheme/fixes). Remaining gate: in-game verification of T4–T11 (pricing/danger against real items; sheet + theme visual pass; T11's OnDemand-focus check — typed filters work, fullscreen Proton survives a panel click), then archive this plan.

note (2026-07-12): T11 — PoE2 retheme (Fontin + bronze/gold tooltip look, rarity-colored names, DESIGN.md/PRODUCT.md added) + fixes: stale-panel race guards, ADR-0007 (KeyboardMode None → OnDemand so min/max + sheet filter accept keystrokes; supersedes ADR-0003), same-item re-check re-prices instead of "No item", truncated-Rare name fallback, rate-limit countdown.

note (2026-07-12, later): rebuilt + reinstalled; **theme user-confirmed in-game** ("much better"). Still open from the T11 gate: OnDemand focus check (type into min/max / sheet filter; fullscreen Proton survives the panel click), same-item re-check, rate-limit countdown behavior.

note (2026-06-28): T8 regex cheat-sheet **disabled for now** — the Ctrl+Alt+F entry point is removed (lib.rs `--regex` branch + installer shortcut), backend (`cheatsheet.rs`/`clipboard.rs`) + Vue panel retained dormant for an easy restore (ADR-0006 stands). Overlay card UI reworked for readability (near-opaque panel, larger/higher-contrast text, labeled filter section).

note (2026-07-02): Ctrl+Alt+F now opens the T9 **rune price sheet** (`--runes`); the game has no clipboard copy on reward tooltips (user-verified), so reward pricing is name-based via poe.ninja. Regex sheet stays dormant on no key.
