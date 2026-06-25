//! Stat-line → trade2 stat-id mapping and item-name → base-type resolution.
//!
//! Reimplements PathofTrading's `StatMapper` + `ItemMapper` (GPLv3, technique
//! reference only — ADR-0001). The trade2 search API filters on opaque stat ids
//! (`explicit.stat_3299347043`), so a copied mod line ("+25 to maximum Life") has to
//! be normalized and matched against the `data/stats` table, with pseudo-total
//! aggregates and reduced→increased / less→more negation handled the way the
//! reference does.

use std::sync::LazyLock;

use regex::Regex;

use super::StatEntry;

static PAREN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\([^)]+\)\s*").unwrap());
static DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" \u{2014} | \u{2013} | \u{2012} | - ").unwrap());
static NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[+-]?\d+(?:\.\d+)?").unwrap());

/// Mod-line normalized forms that map directly to a pseudo aggregate stat (the
/// values traders actually search on). Keyed by normalized text.
const PSEUDO_MAP: &[(&str, &str)] = &[
    ("to maximum life", "pseudo.pseudo_total_life"),
    ("to maximum mana", "pseudo.pseudo_total_mana"),
    ("to maximum energy shield", "pseudo.pseudo_total_energy_shield"),
    ("to fire resistance", "pseudo.pseudo_total_fire_resistance"),
    ("to cold resistance", "pseudo.pseudo_total_cold_resistance"),
    ("to lightning resistance", "pseudo.pseudo_total_lightning_resistance"),
    ("to chaos resistance", "pseudo.pseudo_total_chaos_resistance"),
    (
        "to all elemental resistances",
        "pseudo.pseudo_total_all_elemental_resistances",
    ),
    ("to strength", "pseudo.pseudo_total_strength"),
    ("to dexterity", "pseudo.pseudo_total_dexterity"),
    ("to intelligence", "pseudo.pseudo_total_intelligence"),
    ("to all attributes", "pseudo.pseudo_total_all_attributes"),
    ("increased movement speed", "pseudo.pseudo_increased_movement_speed"),
];

const SYNONYM_MAP: &[(&str, &str)] = &[
    ("chancetodazeonhit", "dazesonhit"),
    ("chancetoblindenemiesonhit", "blindenemiesonhit"),
    ("chancetoigniteonhit", "igniteonhit"),
    ("chancetofreezeonhit", "freezeonhit"),
    ("chancetoshockonhit", "shockonhit"),
];

/// A mod line resolved to a trade2 stat id plus the value used to seed the filter.
#[derive(Debug, Clone)]
pub struct StatMatch {
    pub id: String,
    pub value: Option<f64>,
    pub is_negative: bool,
}

/// Normalize a stat string for fuzzy matching: lowercase, map map/area phrasing,
/// strip everything but `a-z`, drop map/area prefixes and the "uses remaining" tail.
fn normalize(text: &str) -> String {
    let lowered = text
        .to_lowercase()
        .replace("in map", "in your maps")
        .replace("in area", "in your maps");
    let mut clean: String = lowered.chars().filter(char::is_ascii_lowercase).collect();

    const PREFIXES: &[&str] = &[
        "maphas",
        "mapcontains",
        "areahas",
        "areacontains",
        "yourmapshave",
        "yourmapscontain",
    ];
    for p in PREFIXES {
        if let Some(rest) = clean.strip_prefix(p) {
            clean = rest.to_string();
            break;
        }
    }
    if clean.ends_with("usesremaining") || clean.ends_with("useremaining") {
        clean = clean.replace("usesremaining", "").replace("useremaining", "");
    }
    clean
}

/// Maps copied mod lines to trade2 stat ids using the `data/stats` table.
pub struct StatMapper<'a> {
    stats: &'a [StatEntry],
}

impl<'a> StatMapper<'a> {
    pub fn new(stats: &'a [StatEntry]) -> Self {
        StatMapper { stats }
    }

    /// Scan the `data/stats` table for an entry whose normalized text equals
    /// `pattern` (already normalized). Equipment prefers `(local)` ids; non-equipment
    /// prefers non-local; a matching `preferred_source` short-circuits. Returns the
    /// first qualifying id, mirroring the reference's best-match precedence.
    fn lookup(&self, pattern: &str, is_equipment: bool, source: Option<&str>) -> Option<String> {
        let target = normalize(pattern);
        let mut best: Option<String> = None;
        for entry in self.stats {
            let stat_lower = entry.text.to_lowercase();
            if stat_lower.contains("(enchant)") || stat_lower.contains("(fractured)") {
                continue;
            }
            let is_local = stat_lower.contains("(local)");
            let clean_stat = normalize(&entry.text);
            let clean_no_local = if is_local && clean_stat.ends_with("local") {
                clean_stat[..clean_stat.len() - "local".len()].to_string()
            } else {
                clean_stat.clone()
            };
            if clean_stat != target && clean_no_local != target {
                continue;
            }
            let id = &entry.id;
            if let Some(src) = source {
                if id.starts_with(&format!("{src}.")) {
                    if is_equipment && is_local {
                        return Some(id.clone());
                    }
                    if !is_equipment && !is_local {
                        return Some(id.clone());
                    }
                }
            }
            if is_equipment && is_local {
                return Some(id.clone());
            }
            if best.is_none() {
                best = Some(id.clone());
            }
        }
        best
    }

    /// Resolve a single mod line to a [`StatMatch`]. Strips parenthized notes and any
    /// dash-suffix, extracts the leading number as the value, applies the hardcoded
    /// tablet intercepts, optional pseudo aggregation, the api lookup, and the
    /// reduced→increased / less→more / synonym / `chance to` fallbacks.
    pub fn find_trade_id(
        &self,
        line: &str,
        is_equipment: bool,
        source: Option<&str>,
        allow_pseudo: bool,
    ) -> Option<StatMatch> {
        let no_paren = PAREN_RE.replace_all(line, "");
        let clean_line = DASH_RE.split(&no_paren).next().unwrap_or(&no_paren).trim();
        let val: Option<f64> = NUM_RE
            .find(clean_line)
            .and_then(|m| m.as_str().parse().ok());

        let lower_line = clean_line.to_lowercase();
        // Hardcoded intercepts for badly-formatted tablet modifiers (reference parity).
        if lower_line.contains("additional rare chest") {
            return Some(StatMatch::id("explicit.stat_231864447", val));
        }
        if lower_line.contains("chance to contain three additional breaches") {
            return Some(StatMatch::id("explicit.stat_2440265466", val));
        }
        if lower_line.contains("chance to contain an additional breach") {
            return Some(StatMatch::id("explicit.stat_3049505189", val));
        }
        if lower_line.contains("effect of expedition remnants") {
            return Some(StatMatch::id("explicit.stat_3078574625", val));
        }

        let match_pattern = normalize(clean_line);

        if allow_pseudo {
            for (text, pseudo_id) in PSEUDO_MAP {
                if normalize(text) == match_pattern {
                    return Some(StatMatch::id(pseudo_id, Some(val.unwrap_or(1.0))));
                }
            }
        }

        if let Some(id) = self.lookup(&match_pattern, is_equipment, source) {
            return Some(StatMatch::id(&id, val));
        }
        if lower_line.contains("reduced") {
            if let Some(v) = val {
                let alt = match_pattern.replace("reduced", "increased");
                if let Some(id) = self.lookup(&alt, is_equipment, source) {
                    return Some(StatMatch::negative(&id, -v));
                }
            }
        }
        if lower_line.contains("less") {
            if let Some(v) = val {
                let alt = match_pattern.replace("less", "more");
                if let Some(id) = self.lookup(&alt, is_equipment, source) {
                    return Some(StatMatch::negative(&id, -v));
                }
            }
        }
        if let Some((_, syn)) = SYNONYM_MAP.iter().find(|(k, _)| *k == match_pattern) {
            if let Some(id) = self.lookup(syn, is_equipment, source) {
                return Some(StatMatch::id(&id, val));
            }
        }
        if let Some(alt) = match_pattern.strip_prefix("chanceto") {
            let id = self.lookup(alt, is_equipment, source).or_else(|| {
                self.lookup(&alt.replace("onhit", "sonhit"), is_equipment, source)
            });
            if let Some(id) = id {
                return Some(StatMatch::id(&id, val));
            }
        }
        None
    }
}

impl StatMatch {
    fn id(id: &str, value: Option<f64>) -> Self {
        StatMatch {
            id: id.to_string(),
            value,
            is_negative: false,
        }
    }
    fn negative(id: &str, value: f64) -> Self {
        StatMatch {
            id: id.to_string(),
            value: Some(value),
            is_negative: true,
        }
    }
}

/// Resolve a magic item's base type: the longest `data/items` type that is a
/// substring of the name line (the list arrives sorted longest-first). Falls back to
/// the name line itself when nothing matches.
pub fn base_name(items: &[String], name_line: &str) -> String {
    items
        .iter()
        .find(|t| name_line.contains(t.as_str()))
        .cloned()
        .unwrap_or_else(|| name_line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> Vec<StatEntry> {
        vec![
            StatEntry {
                id: "explicit.stat_3299347043".into(),
                text: "# to maximum Life".into(),
            },
            StatEntry {
                id: "explicit.stat_1671376347".into(),
                text: "#% to Lightning Resistance".into(),
            },
            StatEntry {
                id: "explicit.stat_local_attack_speed".into(),
                text: "#% increased Attack Speed (Local)".into(),
            },
            StatEntry {
                id: "explicit.stat_global_attack_speed".into(),
                text: "#% increased Attack Speed".into(),
            },
        ]
    }

    #[test]
    fn pseudo_total_wins_for_life_when_allowed() {
        let s = stats();
        let m = StatMapper::new(&s);
        let r = m
            .find_trade_id("+25 to maximum Life", true, None, true)
            .expect("matches");
        assert_eq!(r.id, "pseudo.pseudo_total_life");
        assert_eq!(r.value, Some(25.0));
    }

    #[test]
    fn local_id_preferred_for_equipment() {
        let s = stats();
        let m = StatMapper::new(&s);
        let r = m
            .find_trade_id("12% increased Attack Speed", true, None, false)
            .expect("matches");
        assert_eq!(r.id, "explicit.stat_local_attack_speed");
        assert_eq!(r.value, Some(12.0));
    }

    #[test]
    fn base_name_picks_longest_substring() {
        let items = vec!["Regalia".to_string(), "Vaal Regalia".to_string()];
        // Caller sorts longest-first; emulate that here.
        let mut sorted = items.clone();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));
        assert_eq!(base_name(&sorted, "Sturdy Vaal Regalia of the Whale"), "Vaal Regalia");
    }
}
