//! Waystone danger-checker (plan T7, ADR-0005).
//!
//! When a price-check copies a **Waystone**, the hotkey path routes here instead of to
//! pricing: this analyzes the waystone's mods against a curated, build-agnostic ruleset
//! and emits a [`DangerReport`] for the overlay (event `price-check-danger`). Pure +
//! synchronous (no network, no GGG quota) so checking many waystones in a row is instant.
//!
//! The ruleset is grounded in the real PoE2 trade2 `data/stats` map-mod phrasings (mined
//! 2026-06-25), NOT PoE1 priors — PoE2 waystones have no reflect / no-regen / -max-res
//! mods; the lethal surface is curses, monster crit/damage/extra-element/projectiles,
//! reduced recovery, and a few status effects. Keyword matching ignores the rolled
//! numbers, so it is robust to value variation. Severities are deliberately simple data
//! to refine; combos escalate to `Deadly`.

use std::collections::HashSet;

use serde::Serialize;

use crate::trade::{display_name, ParsedItem};

/// How dangerous a waystone is overall (and per matched mod). `Ord` runs Safe < Caution
/// < Dangerous < Deadly so the report level is `max` of its flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DangerLevel {
    Safe,
    Caution,
    Dangerous,
    Deadly,
}

/// One matched danger (a mod that hit a rule, or an escalating combination).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DangerFlag {
    pub severity: DangerLevel,
    /// Short danger name, e.g. "Monster crit chance".
    pub label: String,
    /// The mod line that triggered it (empty for combos).
    pub matched: String,
    pub why: String,
}

/// The verdict for a waystone, handed to the overlay (`price-check-danger`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DangerReport {
    pub item: String,
    pub level: DangerLevel,
    pub flags: Vec<DangerFlag>,
}

/// A keyword rule: every keyword (lowercased substring) must be present in a mod line.
struct Rule {
    keywords: &'static [&'static str],
    severity: DangerLevel,
    label: &'static str,
    why: &'static str,
}

use DangerLevel::{Caution, Dangerous, Deadly};

/// Build-agnostic danger rules, keyed on real PoE2 waystone mod phrasings. Beneficial
/// mods ("Players have … more Recovery", "Monsters take increased Damage") match nothing.
const RULES: &[Rule] = &[
    // --- incoming damage amplifiers ---
    Rule { keywords: &["as extra chaos"], severity: Deadly, label: "Extra Chaos damage",
        why: "Monsters add Chaos to every hit — it bypasses Energy Shield and is hard to resist" },
    Rule { keywords: &["as extra fire"], severity: Dangerous, label: "Extra Fire damage",
        why: "Monsters add Fire damage to every hit" },
    Rule { keywords: &["as extra cold"], severity: Dangerous, label: "Extra Cold damage",
        why: "Monsters add Cold damage to every hit" },
    Rule { keywords: &["as extra lightning"], severity: Dangerous, label: "Extra Lightning damage",
        why: "Monsters add Lightning damage to every hit" },
    Rule { keywords: &["monsters deal", "increased damage"], severity: Dangerous, label: "Monsters deal more damage",
        why: "Higher incoming damage across the board" },
    Rule { keywords: &["additional projectiles"], severity: Dangerous, label: "Extra projectiles",
        why: "Overlapping projectile hits can burst you down" },
    Rule { keywords: &["increased critical hit chance"], severity: Dangerous, label: "Monster crit chance",
        why: "Monsters crit far more often — sudden spikes" },
    Rule { keywords: &["critical damage bonus"], severity: Dangerous, label: "Monster crit damage",
        why: "Monster crits hit much harder" },
    Rule { keywords: &["attack, cast and movement speed"], severity: Dangerous, label: "Monster speed",
        why: "Faster attacks, casts and movement — harder to avoid" },
    // --- defensive/sustain debuffs on the player ---
    Rule { keywords: &["less recovery rate"], severity: Dangerous, label: "Reduced recovery",
        why: "Your Life/ES recovery is cut — sustain fails against spike damage" },
    Rule { keywords: &["cursed with elemental weakness"], severity: Dangerous, label: "Elemental Weakness curse",
        why: "Lowers your elemental resistances" },
    Rule { keywords: &["marked for death"], severity: Dangerous, label: "Mark for Death",
        why: "Amplified damage taken after killing a Rare/Unique — can chain-kill you" },
    Rule { keywords: &["delirious"], severity: Dangerous, label: "Delirium",
        why: "Tougher, deadlier Delirium monsters in the area" },
    Rule { keywords: &["deal no damage"], severity: Dangerous, label: "Periodic no-damage",
        why: "You deal no damage 3 of every 10 seconds — you take hits unable to fight back" },
    Rule { keywords: &["cursed with temporal chains"], severity: Caution, label: "Temporal Chains curse",
        why: "Slows your actions, recovery and flask effects" },
    Rule { keywords: &["cursed with enfeeble"], severity: Caution, label: "Enfeeble curse",
        why: "Reduces your damage and accuracy" },
    Rule { keywords: &["reduced flask charges"], severity: Caution, label: "Reduced flask charges",
        why: "Less flask sustain through the map" },
    // --- status effects / control ---
    Rule { keywords: &["poison on hit"], severity: Caution, label: "Poison on hit",
        why: "Stacking Chaos damage-over-time" },
    Rule { keywords: &["bleeding on hit"], severity: Caution, label: "Bleed on hit",
        why: "Physical damage-over-time, worse while moving" },
    Rule { keywords: &["steal", "charges"], severity: Caution, label: "Charge steal",
        why: "Strips your Power/Frenzy/Endurance charges mid-fight" },
    Rule { keywords: &["break armour"], severity: Caution, label: "Armour break",
        why: "Armour builds lose mitigation as the fight goes on" },
    Rule { keywords: &["grasping vine"], severity: Caution, label: "Grasping Vines",
        why: "Roots/slows you in melee range" },
    Rule { keywords: &["flammability"], severity: Caution, label: "Flammability",
        why: "Increases the Fire damage you take" },
    Rule { keywords: &["shock chance"], severity: Caution, label: "Shock",
        why: "Shock increases the damage you take" },
    Rule { keywords: &["freeze buildup"], severity: Caution, label: "Freeze",
        why: "Freeze can lock you in place" },
    Rule { keywords: &["stun buildup"], severity: Caution, label: "Stun",
        why: "Raises stun-lock risk" },
    Rule { keywords: &["increased accuracy"], severity: Caution, label: "Monster accuracy",
        why: "Monsters miss you less often" },
    Rule { keywords: &["increased area of effect"], severity: Caution, label: "Monster AoE",
        why: "Larger monster hit areas — harder to dodge" },
];

/// True if the item is a Waystone (the only class the danger-checker handles; tablets and
/// other map devices are out of scope for T7).
pub fn is_waystone(item: &ParsedItem) -> bool {
    item.item_class == "Waystones"
}

/// Analyze a waystone's mods into a [`DangerReport`]. Each mod is matched against every
/// rule; then a couple of combinations escalate to `Deadly`. A waystone with no matching
/// mods is `Safe`.
pub fn analyze(item: &ParsedItem) -> DangerReport {
    let mut flags: Vec<DangerFlag> = Vec::new();
    for stat in &item.stats {
        let lower = stat.text.to_lowercase();
        for rule in RULES {
            if rule.keywords.iter().all(|k| lower.contains(k)) {
                flags.push(DangerFlag {
                    severity: rule.severity,
                    label: rule.label.to_string(),
                    matched: stat.text.clone(),
                    why: rule.why.to_string(),
                });
            }
        }
    }

    // Combinations that are worse than their parts. Evaluate against the matched labels
    // in a scope so the immutable borrow ends before we push the combo flags.
    let (crit_combo, sustain_combo) = {
        let labels: HashSet<&str> = flags.iter().map(|f| f.label.as_str()).collect();
        // Only the incoming-damage-amplifier flags ("Extra <Element> damage") count here —
        // NOT "Extra projectiles" (more hits, not a damage multiplier), which would otherwise
        // false-escalate reduced-recovery + extra-projectiles to Deadly.
        let has_extra_element = flags
            .iter()
            .any(|f| f.label.starts_with("Extra ") && f.label.ends_with(" damage"));
        (
            labels.contains("Monster crit chance") && labels.contains("Monster crit damage"),
            labels.contains("Reduced recovery")
                && (labels.contains("Monsters deal more damage") || has_extra_element),
        )
    };
    if crit_combo {
        flags.push(DangerFlag {
            severity: Deadly,
            label: "Combo: crit chance + crit damage".to_string(),
            matched: String::new(),
            why: "Monsters crit often AND hit hard — strong one-shot potential".to_string(),
        });
    }
    if sustain_combo {
        flags.push(DangerFlag {
            severity: Deadly,
            label: "Combo: no sustain + amplified damage".to_string(),
            matched: String::new(),
            why: "Your recovery is cut while incoming damage is amplified".to_string(),
        });
    }

    let level = flags
        .iter()
        .map(|f| f.severity)
        .max()
        .unwrap_or(DangerLevel::Safe);

    DangerReport {
        item: display_name(item),
        level,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::parse_item;

    fn waystone(mods: &str) -> ParsedItem {
        parse_item(&format!(
            "Item Class: Waystones\nRarity: Rare\nDread Core\nWaystone (Tier 15)\n--------\nWaystone Tier: 15\nItem Quantity: +40% (augmented)\n--------\n{mods}"
        ))
        .unwrap()
    }

    #[test]
    fn detects_waystone() {
        assert!(is_waystone(&waystone("Monsters deal 30% increased Damage")));
    }

    #[test]
    fn clean_waystone_is_safe() {
        // Beneficial / neutral mods must not flag.
        let r = analyze(&waystone(
            "Players have 20% more Recovery Rate of Life, Mana and Energy Shield\nMonsters take 40% increased Damage",
        ));
        assert_eq!(r.level, DangerLevel::Safe);
        assert!(r.flags.is_empty());
    }

    #[test]
    fn flags_extra_chaos_as_deadly() {
        let r = analyze(&waystone("Monsters deal 15% of Damage as Extra Chaos"));
        assert_eq!(r.level, DangerLevel::Deadly);
        assert!(r.flags.iter().any(|f| f.label == "Extra Chaos damage"));
    }

    #[test]
    fn crit_combo_escalates_to_deadly() {
        let r = analyze(&waystone(
            "Monsters have 200% increased Critical Hit Chance\nMonsters have 30% Critical Damage Bonus",
        ));
        assert_eq!(r.level, DangerLevel::Deadly);
        assert!(r.flags.iter().any(|f| f.label.starts_with("Combo:")));
    }

    #[test]
    fn recovery_plus_projectiles_stays_dangerous() {
        // Extra projectiles is more hits, not a damage amplifier — it must NOT trigger the
        // "no sustain + amplified damage" Deadly combo (label-prefix collision regression).
        let r = analyze(&waystone(
            "Players have 30% less Recovery Rate of Life and Energy Shield\nMonsters fire 2 additional Projectiles",
        ));
        assert_eq!(r.level, DangerLevel::Dangerous);
        assert!(!r.flags.iter().any(|f| f.label.starts_with("Combo:")));
    }

    #[test]
    fn recovery_plus_extra_cold_still_deadly() {
        // The legitimate combo must still escalate.
        let r = analyze(&waystone(
            "Players have 30% less Recovery Rate of Life and Energy Shield\nMonsters deal 12% of Damage as Extra Cold",
        ));
        assert_eq!(r.level, DangerLevel::Deadly);
        assert!(r.flags.iter().any(|f| f.label.starts_with("Combo:")));
    }

    #[test]
    fn curses_and_recovery_flag_at_expected_severity() {
        let r = analyze(&waystone(
            "Players are periodically Cursed with Temporal Chains\nPlayers have 19% less Recovery Rate of Life and Energy Shield",
        ));
        // Temporal Chains is Caution, reduced recovery is Dangerous -> overall Dangerous.
        assert_eq!(r.level, DangerLevel::Dangerous);
        assert!(r.flags.iter().any(|f| f.label == "Reduced recovery"));
        assert!(r.flags.iter().any(|f| f.label == "Temporal Chains curse"));
    }
}
