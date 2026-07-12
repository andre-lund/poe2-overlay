//! Regex cheat-sheet (plan T8, ADR-0006).
//!
//! A curated, build-agnostic library of PoE2 stash/vendor search patterns, surfaced to
//! the overlay (Tauri command `get_cheatsheet`) as a pointer-only click-to-copy list —
//! the overlay took no keyboard focus when this shipped (since relaxed to on-demand
//! focus, ADR-0007), so there is no typed editor; the
//! user clicks a pattern, the app writes it to the X11 clipboard ([`crate::clipboard`]),
//! and pastes it into the game's Ctrl-F box.
//!
//! The patterns are static data (like `danger`'s ruleset), grounded in the PoE2 search
//! syntax confirmed by research (alternation `|`, classes `[]`, ranges, `.`, quantifiers,
//! anchors, the `!` exclusion, space-ANDed blocks; matches the item's full text incl.
//! mods, in stash + vendor windows). Operators are strongly confirmed; the `rarity:` /
//! tier phrasings are version-ambiguous across Early Access patches, so those carry a
//! `verify` note and the user confirms/refines them in-game.

use serde::Serialize;

/// In-game search box length cap. Version-ambiguous across EA patches (older sources say
/// 50; VULKK May-2026 says 250) — kept as one constant + surfaced so the UI can show a
/// length indicator. Verify the live cap on the current patch.
pub const SEARCH_CHAR_LIMIT: usize = 250;

/// One copy-pasteable search pattern.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    pub label: String,
    pub regex: String,
    /// Caveat shown in the UI (empty = strongly confirmed); e.g. "verify on your patch".
    pub note: String,
}

/// A named group of patterns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub name: String,
    pub patterns: Vec<Pattern>,
}

/// The cheat-sheet handed to the overlay: the categorized patterns plus the in-game
/// search length cap (so the UI can surface it).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cheatsheet {
    pub categories: Vec<Category>,
    pub char_limit: usize,
}

/// Build the full cheat-sheet (categories + the search char limit).
pub fn cheatsheet() -> Cheatsheet {
    Cheatsheet {
        categories: categories(),
        char_limit: SEARCH_CHAR_LIMIT,
    }
}

/// (label, regex, note) — `note` empty unless the pattern needs in-game verification.
type Row = (&'static str, &'static str, &'static str);

const DEFENSES: &[Row] = &[
    ("Any resistance", "resistance", ""),
    ("Maximum Life", "maximum life", ""),
    ("Energy Shield", "energy shield", ""),
    ("Life or ES (either)", "maximum life|energy shield", ""),
];

const DAMAGE: &[Row] = &[
    ("Attack Speed", "attack speed", ""),
    ("Critical (any)", "critical", ""),
    ("Spell Damage", "spell damage", ""),
    ("Physical Damage", "physical damage", ""),
];

const RARITY: &[Row] = &[
    ("Rare items only", "rarity:rare", "verify the rarity: prefix on your patch"),
    ("Normal (white) bases", "rarity:normal", "verify the rarity: prefix on your patch"),
    ("Quality 20%", "20% quality", ""),
];

const WAYSTONE: &[Row] = &[
    ("Avoid: Extra Chaos", "extra chaos", "deadly waystone mod — see the danger-check"),
    ("Waystone tier 15+", "tier: 1[5-9]", "verify the waystone tier line wording"),
    ("Has Item Quantity", "item quantity", ""),
];

/// The full cheat-sheet, ordered by category.
pub fn categories() -> Vec<Category> {
    let groups: &[(&str, &[Row])] = &[
        ("Defenses", DEFENSES),
        ("Damage", DAMAGE),
        ("Rarity / quality", RARITY),
        ("Waystone / map", WAYSTONE),
    ];
    groups
        .iter()
        .map(|(name, rows)| Category {
            name: name.to_string(),
            patterns: rows
                .iter()
                .map(|(label, regex, note)| Pattern {
                    label: label.to_string(),
                    regex: regex.to_string(),
                    note: note.to_string(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_are_nonempty_and_within_limit() {
        let cats = categories();
        assert!(!cats.is_empty());
        let mut total = 0;
        for cat in &cats {
            assert!(!cat.patterns.is_empty(), "category {} is empty", cat.name);
            for p in &cat.patterns {
                assert!(!p.label.is_empty());
                assert!(!p.regex.is_empty(), "{} has an empty regex", p.label);
                assert!(
                    p.regex.len() <= SEARCH_CHAR_LIMIT,
                    "{} exceeds the {SEARCH_CHAR_LIMIT}-char search limit",
                    p.label
                );
                total += 1;
            }
        }
        assert!(total >= 10, "expected a starter set of >=10 patterns, got {total}");
    }
}
