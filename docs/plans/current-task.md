plan: active/0001-mvp-price-check-overlay
active: T9 in-game eyeball (rune price sheet on Ctrl+Alt+F). T1–T9 code-complete (MVP price-check + waystone danger-checker + regex cheat-sheet [dormant] + rune sheet). Remaining gate: in-game verification of T4–T9 (pricing/danger against real items; rune sheet visual pass), then archive this plan.

note (2026-06-28): T8 regex cheat-sheet **disabled for now** — the Ctrl+Alt+F entry point is removed (lib.rs `--regex` branch + installer shortcut), backend (`cheatsheet.rs`/`clipboard.rs`) + Vue panel retained dormant for an easy restore (ADR-0006 stands). Overlay card UI reworked for readability (near-opaque panel, larger/higher-contrast text, labeled filter section).

note (2026-07-02): Ctrl+Alt+F now opens the T9 **rune price sheet** (`--runes`); the game has no clipboard copy on reward tooltips (user-verified), so reward pricing is name-based via poe.ninja. Regex sheet stays dormant on no key.
