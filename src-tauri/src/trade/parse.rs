//! PoE2 clipboard item-text parser.
//!
//! Direct reimplementation of PathofTrading's `parse_item` (GPLv3, technique
//! reference only — ADR-0001): split the copied text into lines, pull the header
//! fields (class/rarity/ilvl/quality/sockets/gem level), classify bulk vs gear, and
//! collect the modifier lines with their affix metadata. Pure + synchronous so it is
//! unit-testable; magic-item base-type resolution (which needs the trade2 item list)
//! is deferred to the gear path where the cache lives.

use std::sync::LazyLock;

use regex::Regex;

use super::{ItemStat, ParsedItem};

static LEVEL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Level:\s*(\d+)").unwrap());
static QUALITY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+(\d+)").unwrap());
static TIER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tier: (\d+)").unwrap());

/// Item classes priced in bulk via poe.ninja rather than the trade2 auction.
const BULK_CLASSES: &[&str] = &[
    "Currency",
    "Stackable Currency",
    "Omen",
    "Essence",
    "Fragment",
    "Rune",
    "Idol",
    "Soul Core",
    "Uncut Skill Gem",
    "Uncut Support Gem",
    "Augment",
];

/// Header/section lines that are never modifiers.
const METADATA_KEYWORDS: &[&str] = &[
    "Item Class:",
    "Rarity:",
    "--------",
    "Requirements:",
    "Item Level:",
    "Requires:",
    "Stack Size:",
    "Quality:",
    "Sockets:",
];

/// Colon-bearing lines that ARE modifiers and must not be skipped.
const STAT_PREFIX_EXCEPTIONS: &[&str] = &["Grants Skill:", "Bonded:", "Adds"];

/// Parse PoE2 clipboard item text into a [`ParsedItem`], or `None` if the text is
/// empty. Accepts both basic (Ctrl+C) and advanced (Ctrl+Alt+C, with `{ … Modifier
/// … (Tier: N) }` annotations) copies; tier/source default to explicit when absent.
pub fn parse_item(text: &str) -> Option<ParsedItem> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }

    let mut item = ParsedItem {
        item_class: String::new(),
        rarity: String::new(),
        name: String::new(),
        base_type: String::new(),
        is_bulk: false,
        ilvl: None,
        quality: None,
        sockets: None,
        gem_level: None,
        stats: Vec::new(),
    };

    for line in &lines {
        if let Some(v) = line.strip_prefix("Item Class:") {
            item.item_class = v.trim().to_string();
        }
        if let Some(v) = line.strip_prefix("Rarity:") {
            item.rarity = v.trim().to_string();
        }
        if let Some(v) = line.strip_prefix("Item Level:") {
            item.ilvl = v.trim().parse().ok();
        }
        // Gem level (and, faithfully to the reference, any bare `Level:` requirement
        // line). Guarded off `Item Level:` by the `starts_with` above not matching.
        if line.starts_with("Level:") {
            if let Some(c) = LEVEL_RE.captures(line) {
                item.gem_level = c[1].parse().ok();
            }
        }
        if line.starts_with("Quality:") {
            if let Some(c) = QUALITY_RE.captures(line) {
                item.quality = c[1].parse().ok();
            }
        }
        if let Some(v) = line.strip_prefix("Sockets:") {
            item.sockets = Some(v.chars().filter(char::is_ascii_alphabetic).count() as u32);
        }
    }

    if BULK_CLASSES.contains(&item.item_class.as_str()) || item.rarity.is_empty() {
        // Bulk/stackable: name is the first colon-free line (the currency name).
        item.is_bulk = true;
        item.name = lines
            .iter()
            .find(|l| !l.contains(':'))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        item.base_type = item.name.clone();
        return Some(item);
    }

    // Gear: the name line follows the Rarity line; rares/uniques carry a separate
    // base-type line, magic items embed the base in the name (resolved later).
    match lines.iter().position(|l| l.starts_with("Rarity:")) {
        Some(ridx) => match lines.get(ridx + 1) {
            Some(&name_line) => {
                let rare_or_unique = item.rarity == "Unique" || item.rarity == "Rare";
                if rare_or_unique && lines.get(ridx + 2).is_none() {
                    // Truncated Rare/Unique with no base-type line — the reference's
                    // `except` branch sets both name and base to the first line.
                    item.name = lines[0].to_string();
                    item.base_type = lines[0].to_string();
                } else {
                    item.name = name_line.to_string();
                    item.base_type = if rare_or_unique {
                        lines[ridx + 2].to_string()
                    } else {
                        // Magic (or anything else): base type = name placeholder, refined
                        // by `stats::base_name` in the gear path.
                        name_line.to_string()
                    };
                }
            }
            None => {
                item.name = lines[0].to_string();
                item.base_type = lines[0].to_string();
            }
        },
        None => {
            item.name = lines[0].to_string();
            item.base_type = lines[0].to_string();
        }
    }

    collect_stats(&lines, &mut item);
    Some(item)
}

/// Walk the lines a second time collecting modifier lines, tracking the current
/// affix source/tier from any `{ … Modifier … }` annotation block.
fn collect_stats(lines: &[&str], item: &mut ParsedItem) {
    let mut current_tier: Option<u32> = None;
    let mut current_source = String::from("explicit");

    for line in lines {
        if METADATA_KEYWORDS.iter().any(|k| line.starts_with(k)) {
            continue;
        }
        if line.to_lowercase().contains("uses remaining") {
            continue;
        }
        if line.starts_with('{') && line.contains("Modifier") {
            let tag = line.to_lowercase();
            if tag.contains("implicit") {
                current_source = "implicit".into();
            } else if tag.contains("prefix")
                || tag.contains("suffix")
                || tag.contains("unique")
                || tag.contains("rune")
            {
                current_source = "explicit".into();
            } else if tag.contains("fractured") {
                current_source = "fractured".into();
            } else if tag.contains("enchant") {
                current_source = "enchant".into();
            }
            if let Some(c) = TIER_RE.captures(&tag) {
                current_tier = c[1].parse().ok();
            }
            continue;
        }
        // Skip remaining metadata-style `key: value` lines unless they are one of the
        // colon-bearing modifier forms (Grants Skill / Bonded / Adds).
        if line.contains(':') && !STAT_PREFIX_EXCEPTIONS.iter().any(|k| line.starts_with(k)) {
            continue;
        }
        item.stats.push(ItemStat {
            text: line.to_string(),
            tier: current_tier,
            source: current_source.clone(),
        });
        current_tier = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_currency_as_bulk() {
        let text = "Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n--------\nStack Size: 12/10\n--------\nReroll a Rare item's modifiers";
        let item = parse_item(text).expect("parses");
        assert!(item.is_bulk);
        assert_eq!(item.name, "Chaos Orb");
        assert_eq!(item.base_type, "Chaos Orb");
        assert_eq!(item.rarity, "Currency");
    }

    #[test]
    fn parses_rare_gear_with_base_and_stats() {
        let text = "Item Class: Body Armours\n\
            Rarity: Rare\n\
            Doom Shell\n\
            Vaal Regalia\n\
            --------\n\
            Energy Shield: 200\n\
            --------\n\
            Requirements:\n\
            Int: 159\n\
            --------\n\
            Item Level: 82\n\
            --------\n\
            +25 to maximum Life\n\
            +30% to Fire Resistance";
        let item = parse_item(text).expect("parses");
        assert!(!item.is_bulk);
        assert_eq!(item.rarity, "Rare");
        assert_eq!(item.name, "Doom Shell");
        assert_eq!(item.base_type, "Vaal Regalia");
        assert_eq!(item.ilvl, Some(82));
        let stat_texts: Vec<&str> = item.stats.iter().map(|s| s.text.as_str()).collect();
        assert!(stat_texts.contains(&"+25 to maximum Life"));
        assert!(stat_texts.contains(&"+30% to Fire Resistance"));
        // Energy Shield / Int / Requirements lines are metadata, not stats.
        assert!(!stat_texts.iter().any(|t| t.contains("Energy Shield")));
        assert!(!stat_texts.iter().any(|t| t.contains("Int:")));
    }

    #[test]
    fn empty_text_is_none() {
        assert!(parse_item("   \n  \n").is_none());
    }

    #[test]
    fn truncated_rare_falls_back_to_first_line() {
        // A Rare with no base-type line: mirror the reference's except branch
        // (name == base == lines[0]).
        let item = parse_item("Item Class: Rings\nRarity: Rare\nDoom Coil").expect("parses");
        assert_eq!(item.name, "Item Class: Rings");
        assert_eq!(item.base_type, "Item Class: Rings");
    }
}
