//! poe.ninja bulk pricing (zero GGG quota).
//!
//! Reimplements PathofTrading's poe.ninja path (GPLv3, technique reference only —
//! ADR-0001). Exchange rates come from the Currency exchange overview, normalized to
//! exalt-equivalents; an individual bulk item is looked up first in the cached rates,
//! then in its on-demand category overview (essences, runes, omens, …).

use std::collections::HashMap;

use serde::Deserialize;

use super::{round2, Listing, ParsedItem};

#[derive(Deserialize)]
struct Overview {
    /// `core.rates.exalted` = exalts per primary display unit (divine today) — the
    /// same converter the item overview carries. `primaryValue` everywhere in this
    /// API is denominated in that primary unit, NOT in exalts (the original T4
    /// assumption — poe.ninja's primary was exalted then, divine now).
    #[serde(default)]
    core: OverviewCore,
    #[serde(default)]
    lines: Vec<Line>,
    /// Item metadata keyed by id — the real names (with apostrophes) that `lines`
    /// stopped carrying.
    #[serde(default)]
    items: Vec<OverviewItem>,
}

#[derive(Deserialize, Default)]
struct OverviewCore {
    #[serde(default)]
    rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct Line {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "primaryValue")]
    primary_value: Option<f64>,
}

#[derive(Deserialize)]
struct OverviewItem {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
}

impl Overview {
    /// Exalts per `primaryValue` unit, from the response's own converter; falls back
    /// to the cached divine exchange rate (the primary unit is divine today).
    fn exalts_per_primary(&self, rates: &HashMap<String, f64>) -> f64 {
        self.core
            .rates
            .get("exalted")
            .copied()
            .filter(|v| *v > 0.0)
            .unwrap_or_else(|| rates.get("divine").copied().unwrap_or(193.0))
    }
}

fn overview_url() -> &'static str {
    "https://poe.ninja/poe2/api/economy/exchange/current/overview"
}

/// Common-currency item name → poe.ninja exchange id. poe.ninja keys the high-value
/// orbs by short aliases (`divine`, not `divine-orb`) and returns `null` names, so a
/// name-derived dash-id misses them entirely. The reference defines this map but
/// never wires it in (dead code) — so common currency silently falls through to the
/// GGG auction there; we use it, keeping bulk currency on the zero-quota path
/// (ADR-0001, ADR-0004). Long-named orbs (Orb of Extraction → `orb-of-extraction`)
/// match the dash-id fallback directly and need no entry.
const CURRENCY_MAP: &[(&str, &str)] = &[
    ("Chaos Orb", "chaos"),
    ("Exalted Orb", "exalted"),
    ("Divine Orb", "divine"),
    ("Orb of Alchemy", "alch"),
    ("Orb of Transmutation", "transmute"),
    ("Orb of Augmentation", "aug"),
    ("Orb of Annulment", "annul"),
    ("Regal Orb", "regal"),
    ("Vaal Orb", "vaal"),
    ("Gemcutter's Prism", "gcp"),
    ("Glassblower's Bauble", "bauble"),
    ("Blacksmith's Whetstone", "whetstone"),
    ("Armourer's Scrap", "scrap"),
    ("Mirror of Kalandra", "mirror"),
    ("Orb of Chance", "chance"),
    ("Artificer's Orb", "artificers"),
    ("Orb of Extraction", "orb-of-extraction"),
    ("Arcanist's Etcher", "etcher"),
    ("Scroll of Wisdom", "wisdom"),
    ("Fracturing Orb", "fracturing-orb"),
    ("Hinekora's Lock", "hinekoras-lock"),
    ("Lesser Jeweller's Orb", "lesser-jewellers-orb"),
    ("Greater Jeweller's Orb", "greater-jewellers-orb"),
    ("Perfect Jeweller's Orb", "perfect-jewellers-orb"),
    ("Crystallised Corruption", "crystallised-corruption"),
];

/// Fetch the Currency exchange overview and return rate updates as exalt-equivalents
/// (`exalted` → 1.0), including the `exalt`/`alch` aliases the reference keys on.
/// Best-effort: `None` on any failure leaves the seeded/cached rates in place.
pub async fn fetch_exchange_rates(
    client: &reqwest::Client,
    league: &str,
) -> Option<HashMap<String, f64>> {
    let resp = client
        .get(overview_url())
        .query(&[("league", league), ("type", "Currency")])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Overview = resp.json().await.ok()?;
    // All values are denominated against exalted's value, so dividing by it yields
    // exalt-equivalents (exalted → 1.0, divine → ~193, …).
    let exalt_base = body
        .lines
        .iter()
        .find(|l| l.id == "exalted")
        .and_then(|l| l.primary_value)
        .filter(|v| *v > 0.0)?;

    let mut rates = HashMap::new();
    for line in &body.lines {
        let Some(val) = line.primary_value else {
            continue;
        };
        if val <= 0.0 {
            continue;
        }
        let ev = val / exalt_base;
        let id = line.id.to_lowercase();
        if id == "exalted" {
            rates.insert("exalt".to_string(), ev);
        }
        if id == "orb-of-alchemy" {
            rates.insert("alch".to_string(), ev);
        }
        rates.insert(id, ev);
    }
    Some(rates)
}

/// poe.ninja category overviews to probe for a bulk item, chosen from its name/class.
fn types_to_check(item: &ParsedItem) -> &'static [&'static str] {
    let name = item.name.to_lowercase();
    if name.contains("omen") {
        &["Ritual"]
    } else if name.contains("essence") {
        &["Essences"]
    } else if name.contains("soul core") {
        &["SoulCores"]
    } else if name.contains("rune") {
        &["Runes"]
    } else if name.contains("idol") {
        &["Idols"]
    } else if name.contains("uncut") {
        &["UncutGems"]
    } else if name.contains("lineage") {
        &["LineageSupportGems"]
    } else if name.contains("abyss") {
        &["Abyss"]
    } else if name.contains("breach") {
        &["Breach"]
    } else if name.contains("expedition") {
        &["Expedition"]
    } else if name.contains("delirium") {
        &["Delirium"]
    } else if item.item_class.to_lowercase().contains("fragment") || name.contains("splinter") {
        &["Fragments"]
    } else {
        &[
            "Currency",
            "Fragments",
            "Essences",
            "Ritual",
            "SoulCores",
            "Runes",
            "Expedition",
            "Delirium",
            "Breach",
            "Abyss",
            "Idols",
        ]
    }
}

/// Price a bulk item from poe.ninja. Returns `None` (caller falls through to the
/// trade2 auction) when poe.ninja has no value for it.
pub async fn price_bulk(
    client: &reqwest::Client,
    league: &str,
    item: &ParsedItem,
    rates: &HashMap<String, f64>,
) -> Option<Listing> {
    let name_lower = item.name.to_lowercase();
    let ninja_id = CURRENCY_MAP
        .iter()
        .find(|(name, _)| *name == item.name)
        .map(|(_, id)| id.to_string())
        .unwrap_or_else(|| name_lower.replace(' ', "-").replace('\'', ""));

    let mut exalt_val = rates.get(&ninja_id).copied();

    // Not in the Currency cache — probe the relevant category overview(s). Like every
    // overview in this API, per-category `primaryValue` is denominated in the primary
    // display unit (divine today) and converts via the response's own exalted rate.
    if exalt_val.is_none() {
        'outer: for t in types_to_check(item) {
            let Ok(resp) = client
                .get(overview_url())
                .query(&[("league", league), ("type", t)])
                .send()
                .await
            else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            let Ok(body) = resp.json::<Overview>().await else {
                continue;
            };
            for line in &body.lines {
                // poe.ninja returns null names in `lines` today, so the id match is
                // what hits; the name comparison is kept in case names repopulate.
                if line.id == ninja_id || line.name.to_lowercase() == name_lower {
                    if let Some(v) = line.primary_value {
                        exalt_val = Some(v * body.exalts_per_primary(rates));
                        break 'outer;
                    }
                }
            }
        }
    }

    let exalt_val = exalt_val?;
    let divine = rates.get("divine").copied().unwrap_or(193.0);
    let display = if ninja_id == "exalted" {
        "1 E".to_string()
    } else if ninja_id == "divine" {
        "1 D".to_string()
    } else {
        display_value(exalt_val, divine)
    };

    Some(Listing {
        display,
        exalt_val,
        age: "poe.ninja avg".to_string(),
    })
}

/// Exalt-or-divine price display shared by bulk pricing and the rune sheet.
fn display_value(exalt_val: f64, divine: f64) -> String {
    if exalt_val >= divine {
        format!("{} D", round2(exalt_val / divine))
    } else {
        format!("{} E", round2(exalt_val))
    }
}

/// One entry on the category price sheet (T9). `name` is derived from the poe.ninja
/// id — the API returns null names — so apostrophes are lost ("Craiceanns", not
/// "Craiceann's"); good enough to eyeball against an in-game tooltip.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetEntry {
    pub name: String,
    pub display: String,
    pub exalt_val: f64,
}

/// Where a sheet category's data comes from: the currency exchange overview (bulk
/// stackables) or the stash item overview (unique equipment, tablets).
#[derive(Clone, Copy, PartialEq)]
pub enum SheetSource {
    Exchange,
    Items,
}

/// One price-sheet category: (UI label, poe.ninja overview type, endpoint). Labels
/// are globally unique.
pub type SheetCategory = (&'static str, &'static str, SheetSource);

/// One group of price-sheet categories (a tab on the sheet panel).
pub struct SheetGroup {
    pub name: &'static str,
    pub categories: &'static [SheetCategory],
}

/// The price-sheet catalogue. First group's first category is the default panel on
/// Ctrl+Alt+F. The Exchange categories mirror the overview types `types_to_check`
/// probes for single-item bulk pricing (Abyssal Bones live under "Abyss"); the Items
/// ones come from the item overview instead. Expedition sits under Atlas (logbooks,
/// sagas, fluxes are atlas content) even though it is exchange-sourced.
pub const SHEET_GROUPS: &[SheetGroup] = &[
    SheetGroup {
        name: "General",
        categories: &[
            ("Runes", "Runes", SheetSource::Exchange),
            ("Currency", "Currency", SheetSource::Exchange),
            ("Fragments", "Fragments", SheetSource::Exchange),
            ("Essences", "Essences", SheetSource::Exchange),
            ("Omens", "Ritual", SheetSource::Exchange),
            ("Soul Cores", "SoulCores", SheetSource::Exchange),
            ("Abyss", "Abyss", SheetSource::Exchange),
            ("Breach", "Breach", SheetSource::Exchange),
            ("Delirium", "Delirium", SheetSource::Exchange),
            ("Idols", "Idols", SheetSource::Exchange),
        ],
    },
    // poe.ninja tracks no PoE2 skill-gem market — uncut + lineage support gems are
    // the whole gem economy it prices.
    SheetGroup {
        name: "Gems",
        categories: &[
            ("Uncut Gems", "UncutGems", SheetSource::Exchange),
            ("Lineage Gems", "LineageSupportGems", SheetSource::Exchange),
        ],
    },
    SheetGroup {
        name: "Equipment",
        categories: &[
            ("Unique Weapons", "UniqueWeapons", SheetSource::Items),
            ("Unique Armours", "UniqueArmours", SheetSource::Items),
            ("Unique Accessories", "UniqueAccessories", SheetSource::Items),
            ("Unique Jewels", "UniqueJewels", SheetSource::Items),
            ("Unique Flasks", "UniqueFlasks", SheetSource::Items),
        ],
    },
    SheetGroup {
        name: "Atlas",
        categories: &[
            ("Precursor Tablets", "PrecursorTablets", SheetSource::Items),
            ("Unique Tablets", "UniqueTablets", SheetSource::Items),
            ("Expedition", "Expedition", SheetSource::Exchange),
        ],
    },
];

/// Fetch one poe.ninja exchange category overview for the price sheet, most valuable
/// first. `primaryValue` is divine-denominated (like everything in this API) and
/// converts via the response's own exalted rate. Real names come from the response's
/// `items` metadata, with the id-derived fallback. Best-effort: `None` on any failure
/// — the sheet renders a retry hint.
pub async fn fetch_price_sheet(
    client: &reqwest::Client,
    league: &str,
    rates: &HashMap<String, f64>,
    ninja_type: &str,
) -> Option<Vec<SheetEntry>> {
    let resp = client
        .get(overview_url())
        .query(&[("league", league), ("type", ninja_type)])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Overview = resp.json().await.ok()?;
    let divine = rates.get("divine").copied().unwrap_or(193.0);
    let per_primary = body.exalts_per_primary(rates);
    let names: HashMap<&str, &str> = body
        .items
        .iter()
        .filter(|i| !i.name.is_empty())
        .map(|i| (i.id.as_str(), i.name.as_str()))
        .collect();

    let mut entries: Vec<SheetEntry> = body
        .lines
        .iter()
        .filter_map(|l| {
            let v = l.primary_value.filter(|v| *v > 0.0)? * per_primary;
            Some(SheetEntry {
                name: names
                    .get(l.id.as_str())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| sheet_name(&l.id)),
                display: display_value(v, divine),
                exalt_val: v,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.exalt_val.total_cmp(&a.exalt_val));
    (!entries.is_empty()).then_some(entries)
}

fn item_overview_url() -> &'static str {
    "https://poe.ninja/poe2/api/economy/stash/current/item/overview"
}

#[derive(Deserialize)]
struct ItemOverview {
    #[serde(default)]
    core: OverviewCore,
    #[serde(default)]
    lines: Vec<ItemLine>,
}

#[derive(Deserialize)]
struct ItemLine {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "baseType")]
    base_type: String,
    #[serde(default, rename = "detailsId")]
    details_id: String,
    #[serde(default)]
    corrupted: bool,
    #[serde(default, rename = "primaryValue")]
    primary_value: Option<f64>,
}

/// Fetch a poe.ninja *item* overview (unique equipment, tablets) for the price sheet,
/// most valuable first. Denominated like the exchange overviews: divine-based
/// `primaryValue`, converted via `core.rates.exalted` with the cached divine rate as
/// fallback. Best-effort: `None` on any failure.
pub async fn fetch_item_sheet(
    client: &reqwest::Client,
    league: &str,
    rates: &HashMap<String, f64>,
    ninja_type: &str,
) -> Option<Vec<SheetEntry>> {
    let resp = client
        .get(item_overview_url())
        .query(&[("league", league), ("type", ninja_type)])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: ItemOverview = resp.json().await.ok()?;
    let divine = rates.get("divine").copied().unwrap_or(193.0);
    let per_primary = body
        .core
        .rates
        .get("exalted")
        .copied()
        .filter(|v| *v > 0.0)
        .unwrap_or(divine);

    let mut entries: Vec<SheetEntry> = body
        .lines
        .iter()
        .filter_map(|l| {
            let v = l.primary_value.filter(|v| *v > 0.0)? * per_primary;
            Some(SheetEntry {
                name: item_entry_name(l),
                display: display_value(v, divine),
                exalt_val: v,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.exalt_val.total_cmp(&a.exalt_val));
    (!entries.is_empty()).then_some(entries)
}

/// Row label for an item-overview line. poe.ninja splits an item into one line per
/// variant, so the label carries what distinguishes them: the base type when it
/// differs from the name (a unique on two bases), the rarity suffix from `detailsId`
/// (tablets get one line per normal/magic/rare — normal stays unmarked), and the
/// corruption flag.
fn item_entry_name(l: &ItemLine) -> String {
    let mut name = if !l.base_type.is_empty() && l.base_type != l.name {
        format!("{} ({})", l.name, l.base_type)
    } else {
        l.name.clone()
    };
    for rarity in ["magic", "rare"] {
        if l.details_id.ends_with(&format!("-{rarity}")) {
            name.push_str(&format!(" [{rarity}]"));
        }
    }
    if l.corrupted {
        name.push_str(" [corrupted]");
    }
    name
}

/// Entry name for a poe.ninja id: the Currency overview keys high-value orbs by short
/// aliases (`divine`, `alch`), so the reverse of `CURRENCY_MAP` restores the real
/// name; everything else title-cases the dash-id.
fn sheet_name(id: &str) -> String {
    CURRENCY_MAP
        .iter()
        .find(|(_, ninja_id)| *ninja_id == id)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| name_from_id(id))
}

/// Human name from a poe.ninja dash-id: `craiceanns-rune-of-recovery` →
/// `Craiceanns Rune of Recovery` (connectives stay lowercase).
fn name_from_id(id: &str) -> String {
    id.split('-')
        .enumerate()
        .map(|(i, w)| {
            if i > 0 && matches!(w, "of" | "the" | "and") {
                w.to_string()
            } else {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_from_id_title_cases_with_lowercase_connectives() {
        assert_eq!(
            name_from_id("craiceanns-rune-of-recovery"),
            "Craiceanns Rune of Recovery"
        );
        assert_eq!(name_from_id("adept-rune"), "Adept Rune");
        assert_eq!(name_from_id("of-the-x"), "Of the X"); // leading connective still capitalized
    }

    #[test]
    fn item_entry_name_tags_variants() {
        let line = |name: &str, base: &str, details: &str, corrupted: bool| ItemLine {
            name: name.into(),
            base_type: base.into(),
            details_id: details.into(),
            corrupted,
            primary_value: Some(1.0),
        };
        // unique on a base: base type shown
        assert_eq!(
            item_entry_name(&line("Bluetongue", "Shortsword", "bluetongue-shortsword", false)),
            "Bluetongue (Shortsword)"
        );
        // tablet rarity variants: normal unmarked, magic/rare tagged
        assert_eq!(
            item_entry_name(&line(
                "Ritual Tablet",
                "Ritual Tablet",
                "ritual-tablet-ritual-tablet-normal",
                false
            )),
            "Ritual Tablet"
        );
        assert_eq!(
            item_entry_name(&line(
                "Ritual Tablet",
                "Ritual Tablet",
                "ritual-tablet-ritual-tablet-rare",
                false
            )),
            "Ritual Tablet [rare]"
        );
        assert_eq!(
            item_entry_name(&line("X", "X", "x-x-magic", true)),
            "X [magic] [corrupted]"
        );
    }

    #[test]
    fn sheet_name_restores_currency_aliases() {
        assert_eq!(sheet_name("divine"), "Divine Orb"); // short alias → CURRENCY_MAP reverse
        assert_eq!(sheet_name("adept-rune"), "Adept Rune"); // non-currency → dash-id title case
    }
}
