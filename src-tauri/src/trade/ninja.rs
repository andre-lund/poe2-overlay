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
    #[serde(default)]
    lines: Vec<Line>,
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

    // Not in the Currency cache — probe the relevant category overview(s). Unlike the
    // divine-based Currency overview (normalized in `fetch_exchange_rates`), per-category
    // `primaryValue` is already exalt-denominated, so it is used directly.
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
                // poe.ninja returns null names today, so the id match is what hits;
                // the name comparison is kept in case names are repopulated.
                if line.id == ninja_id || line.name.to_lowercase() == name_lower {
                    if let Some(v) = line.primary_value {
                        exalt_val = Some(v);
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

/// One rune on the price sheet (T9). `name` is derived from the poe.ninja id —
/// the API returns null names — so apostrophes are lost ("Craiceanns", not
/// "Craiceann's"); good enough to eyeball against an in-game reward tooltip.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuneEntry {
    pub name: String,
    pub display: String,
    pub exalt_val: f64,
}

/// Fetch the poe.ninja Runes overview for the rune price sheet, most valuable first.
/// Per-category `primaryValue` is already exalt-denominated (same as `price_bulk`'s
/// probe). Best-effort: `None` on any failure — the sheet renders a retry hint.
pub async fn fetch_rune_sheet(
    client: &reqwest::Client,
    league: &str,
    rates: &HashMap<String, f64>,
) -> Option<Vec<RuneEntry>> {
    let resp = client
        .get(overview_url())
        .query(&[("league", league), ("type", "Runes")])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Overview = resp.json().await.ok()?;
    let divine = rates.get("divine").copied().unwrap_or(193.0);

    let mut entries: Vec<RuneEntry> = body
        .lines
        .iter()
        .filter_map(|l| {
            let v = l.primary_value.filter(|v| *v > 0.0)?;
            Some(RuneEntry {
                name: name_from_id(&l.id),
                display: display_value(v, divine),
                exalt_val: v,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.exalt_val.total_cmp(&a.exalt_val));
    (!entries.is_empty()).then_some(entries)
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
}
