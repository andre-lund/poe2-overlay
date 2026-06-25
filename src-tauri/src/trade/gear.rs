//! GGG trade2 gear pricing — search + fetch.
//!
//! Reimplements PathofTrading's `fetch_real_price` gear path (GPLv3, technique
//! reference only — ADR-0001): turn the parsed item into trade2 stat + base-property
//! filters, POST `/api/trade2/search/poe2/<league>`, then GET `/api/trade2/fetch/`
//! for the cheapest listings — recording the `X-Rate-Limit` headers on both calls so
//! the next check self-throttles before GGG IP-bans us.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::stats::{base_name, StatMapper};
use super::{
    display_name, BaseProp, CacheSnapshot, Listing, ParsedItem, ParsedStat, PriceResult,
    PriceStatus, RateLimit,
};

const SEARCH_BASE: &str = "https://www.pathofexile.com/api/trade2/search/poe2/";
const FETCH_BASE: &str = "https://www.pathofexile.com/api/trade2/fetch/";

/// Explicit single-element resistance stat ids, summed into a pseudo total.
const ELE_IDS: &[&str] = &["stat_3372524247", "stat_4220027924", "stat_1671376347"];
/// "to all Elemental Resistances" — counts triple toward the elemental total.
const ALL_RES_ID: &str = "stat_2901986750";

/// Map a PoE2 item class to its trade2 category id (`""` → no mapping).
fn category_for(class: &str) -> Option<&'static str> {
    Some(match class {
        "Gloves" => "armour.gloves",
        "Body Armours" => "armour.chest",
        "Helmets" => "armour.helmet",
        "Boots" => "armour.boots",
        "Amulets" => "accessory.amulet",
        "Rings" => "accessory.ring",
        "Belts" => "accessory.belt",
        "Quivers" => "armour.quiver",
        "Shields" => "armour.shield",
        "Foci" | "Focus" => "armour.focus",
        "Bucklers" | "Buckler" => "armour.buckler",
        "Bows" => "weapon.bow",
        "Crossbows" => "weapon.crossbow",
        "Wands" => "weapon.wand",
        "Sceptres" => "weapon.sceptre",
        "Staves" => "weapon.staff",
        "Two Hand Maces" => "weapon.twomace",
        "Two Hand Swords" => "weapon.twosword",
        "Two Hand Axes" => "weapon.twoaxe",
        "One Hand Maces" => "weapon.onemace",
        "One Hand Swords" => "weapon.onesword",
        "One Hand Axes" => "weapon.oneaxe",
        "Claws" => "weapon.claw",
        "Daggers" => "weapon.dagger",
        "Spears" => "weapon.spear",
        "Flails" => "weapon.flail",
        "Quarterstaves" => "weapon.warstaff",
        "Life Flasks" => "flask.life",
        "Mana Flasks" => "flask.mana",
        "Jewels" => "jewel",
        "Waystones" => "map.waystone",
        "Tablet" => "map.tablet",
        _ => return None,
    })
}

/// Price gear/waystones via the trade2 auction. Builds the stat + base-property
/// filters from the parsed item, then runs the query. Always returns a renderable
/// result; network/quota failures become an `Empty`/`Error`/`RateLimited` status.
pub async fn price_gear(
    client: &reqwest::Client,
    league: &str,
    item: &ParsedItem,
    snapshot: &CacheSnapshot,
    rate: &Mutex<RateLimit>,
) -> PriceResult {
    let cat_id = category_for(&item.item_class).unwrap_or("");
    let is_equip = cat_id.starts_with("armour.") || cat_id.starts_with("weapon.");

    // Base type for non-Unique/non-Rare items was deferred from the parser (it needs
    // the item-type list). Covers Magic affixes AND the Normal-rarity "Superior "
    // quality prefix — faithful to the reference's else branch. base_name falls back to
    // the verbatim name line when nothing matches, so plain white items are unaffected.
    let base_type = if item.rarity == "Unique" || item.rarity == "Rare" {
        item.base_type.clone()
    } else {
        base_name(&snapshot.items, &item.name)
    };

    let mapper = StatMapper::new(&snapshot.stats);
    let parsed_stats = build_parsed_stats(&mapper, item, is_equip);
    let base_properties = build_base_properties(item, &base_type);

    run_gear_query(
        client,
        league,
        display_name(item),
        parsed_stats,
        base_properties,
        snapshot,
        rate,
    )
    .await
}

/// Run a trade2 search+fetch from already-built stat + base-property filters — the
/// shared core of `price_gear` and the T5 requery command (which supplies the user's
/// edited filters). Records the `X-Rate-Limit` headers on both calls.
#[allow(clippy::too_many_arguments)]
pub async fn run_gear_query(
    client: &reqwest::Client,
    league: &str,
    name: String,
    parsed_stats: Vec<ParsedStat>,
    base_properties: Vec<BaseProp>,
    snapshot: &CacheSnapshot,
    rate: &Mutex<RateLimit>,
) -> PriceResult {
    let query = build_query(&parsed_stats, &base_properties);

    // --- search ---
    let search_url = format!("{SEARCH_BASE}{}", urlencoding::encode(league));
    let resp = match client.post(&search_url).json(&query).send().await {
        Ok(r) => r,
        Err(e) => {
            return result(
                &name,
                league,
                PriceStatus::Error,
                Some(format!("Search failed: {e}")),
                Vec::new(),
                parsed_stats,
                base_properties,
                snapshot,
            )
        }
    };
    rate.lock().unwrap_or_else(|e| e.into_inner()).apply_headers(resp.headers());

    let status = resp.status();
    if status.as_u16() == 429 {
        // Arm the lockout to GGG's actual Retry-After penalty, not just the
        // window/limit estimate, so the next check waits it out (ADR-0004 IP-ban safety).
        let retry = retry_after_secs(&resp);
        rate.lock().unwrap_or_else(|e| e.into_inner()).arm_secs(retry);
        return result(
            &name,
            league,
            PriceStatus::RateLimited,
            Some(format!("Rate limit exceeded — try again in {retry}s")),
            Vec::new(),
            parsed_stats,
            base_properties,
            snapshot,
        );
    }
    if status.as_u16() == 400 {
        return result(
            &name,
            league,
            PriceStatus::Error,
            Some("Invalid search (check base type)".into()),
            Vec::new(),
            parsed_stats,
            base_properties,
            snapshot,
        );
    }
    if !status.is_success() {
        return result(
            &name,
            league,
            PriceStatus::Error,
            Some(format!("Search error {}", status.as_u16())),
            Vec::new(),
            parsed_stats,
            base_properties,
            snapshot,
        );
    }

    let data: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return result(
                &name,
                league,
                PriceStatus::Error,
                Some(format!("Bad search response: {e}")),
                Vec::new(),
                parsed_stats,
                base_properties,
                snapshot,
            )
        }
    };

    let result_ids: Vec<String> = data
        .get("result")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let query_id = data.get("id").and_then(|v| v.as_str()).unwrap_or_default();

    if result_ids.is_empty() {
        return result(
            &name,
            league,
            PriceStatus::Empty,
            Some("No matching listings".into()),
            Vec::new(),
            parsed_stats,
            base_properties,
            snapshot,
        );
    }

    // --- fetch (cheapest 10) ---
    let batch = result_ids[..result_ids.len().min(10)].join(",");
    let fetch_url = format!("{FETCH_BASE}{batch}");
    let resp = match client
        .get(&fetch_url)
        .query(&[("query", query_id)])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return result(
                &name,
                league,
                PriceStatus::Error,
                Some(format!("Fetch failed: {e}")),
                Vec::new(),
                parsed_stats,
                base_properties,
                snapshot,
            )
        }
    };
    rate.lock().unwrap_or_else(|e| e.into_inner()).apply_headers(resp.headers());

    if resp.status().as_u16() == 429 {
        let retry = retry_after_secs(&resp);
        rate.lock().unwrap_or_else(|e| e.into_inner()).arm_secs(retry);
        return result(
            &name,
            league,
            PriceStatus::RateLimited,
            Some(format!("Fetch limit exceeded — wait {retry}s")),
            Vec::new(),
            parsed_stats,
            base_properties,
            snapshot,
        );
    }

    let listings = match resp.json::<Value>().await {
        Ok(body) => normalize_listings(&body, &snapshot.rates),
        Err(_) => Vec::new(),
    };

    let status = if listings.is_empty() {
        PriceStatus::Empty
    } else {
        PriceStatus::Success
    };
    let message = listings.is_empty().then(|| "No priced listings".to_string());
    result(
        &name,
        league,
        status,
        message,
        listings,
        parsed_stats,
        base_properties,
        snapshot,
    )
}

/// Two-pass stat mapping: pass one aggregates element resistances into a pseudo
/// total; pass two resolves every other line (preferring pseudo aggregates).
fn build_parsed_stats(mapper: &StatMapper, item: &ParsedItem, is_equip: bool) -> Vec<ParsedStat> {
    let mut out: Vec<ParsedStat> = Vec::new();
    let mut ele_sum = 0.0_f64;
    let mut best_ele_tier: Option<u32> = None;

    for stat in &item.stats {
        let source = Some(stat.source.as_str());
        // Pass 1: detect single/all elemental resistance for aggregation.
        if let Some(res) = mapper.find_trade_id(&stat.text, is_equip, source, false) {
            let sid = res.id.rsplit('.').next().unwrap_or(&res.id);
            if ELE_IDS.contains(&sid) {
                ele_sum += res.value.unwrap_or(0.0);
                best_ele_tier = better_tier(best_ele_tier, stat.tier);
                continue;
            }
            if sid == ALL_RES_ID {
                ele_sum += res.value.unwrap_or(0.0) * 3.0;
                best_ele_tier = better_tier(best_ele_tier, stat.tier);
                continue;
            }
        }
        // Pass 2: final mapping (pseudo aggregates allowed).
        if let Some(res) = mapper.find_trade_id(&stat.text, is_equip, source, true) {
            let is_neg = res.is_negative || res.value.is_some_and(|v| v < 0.0);
            let cval = weighted_bound(&stat.text, res.value);
            out.push(ParsedStat {
                id: res.id,
                text: stat.text.clone(),
                tier: stat.tier,
                value: res.value,
                min: if is_neg { String::new() } else { cval.clone() },
                max: if is_neg { cval } else { String::new() },
                active: true,
            });
        }
    }

    if ele_sum > 0.0 {
        let cval = (ele_sum * 0.8).round();
        out.insert(
            0,
            ParsedStat {
                id: "pseudo.pseudo_total_elemental_resistance".to_string(),
                text: format!("+{}% total Elemental Resistance", ele_sum as i64),
                tier: best_ele_tier,
                value: Some(ele_sum),
                min: format!("{}", cval as i64),
                max: String::new(),
                active: true,
            },
        );
    }
    out
}

/// Lower tier number = better roll; keep the smallest seen.
fn better_tier(current: Option<u32>, candidate: Option<u32>) -> Option<u32> {
    match (current, candidate) {
        (Some(c), Some(n)) => Some(c.min(n)),
        (None, n) => n,
        (c, None) => c,
    }
}

/// Seed bound for a stat filter: 80% of the rolled value, but the full value for
/// granted-skill / flat-added lines. Empty when the line carries no number.
fn weighted_bound(line: &str, value: Option<f64>) -> String {
    match value {
        None => String::new(),
        Some(v) => {
            let full = line.contains("Grants Skill:")
                || line.contains("Bonded:")
                || line.contains("Adds");
            let mult = if full { 1.0 } else { 0.8 };
            format!("{}", (v * mult).round() as i64)
        }
    }
}

/// Build the toggleable base-property list with the reference's default active flags.
fn build_base_properties(item: &ParsedItem, base_type: &str) -> Vec<BaseProp> {
    let is_magic_rare = item.rarity == "Magic" || item.rarity == "Rare";
    let mut props = vec![
        BaseProp {
            id: "class".into(),
            text: item.item_class.clone(),
            value: item.item_class.clone(),
            active: is_magic_rare,
        },
        BaseProp {
            id: "rarity".into(),
            text: item.rarity.clone(),
            value: item.rarity.clone(),
            active: false,
        },
        BaseProp {
            id: "base".into(),
            text: base_type.to_string(),
            value: base_type.to_string(),
            active: !is_magic_rare,
        },
    ];
    if item.rarity == "Unique" {
        props.push(BaseProp {
            id: "name".into(),
            text: item.name.clone(),
            value: item.name.clone(),
            active: true,
        });
    }
    if let Some(ilvl) = item.ilvl {
        props.push(BaseProp {
            id: "ilvl".into(),
            text: format!("iLvl {ilvl}"),
            value: ilvl.to_string(),
            active: false,
        });
    }
    if let Some(gl) = item.gem_level {
        props.push(BaseProp {
            id: "gemLevel".into(),
            text: format!("Level {gl}"),
            value: gl.to_string(),
            active: true,
        });
    }
    if let Some(q) = item.quality {
        props.push(BaseProp {
            id: "quality".into(),
            text: format!("+{q}% Quality"),
            value: q.to_string(),
            active: false,
        });
    }
    if let Some(s) = item.sockets {
        let is_gem = item.item_class == "Skill Gems" || item.item_class == "Support Gems";
        props.push(BaseProp {
            id: if is_gem { "gem_sockets" } else { "sockets" }.into(),
            text: format!("{s} Sockets"),
            value: s.to_string(),
            active: false,
        });
    }
    props
}

/// Assemble the trade2 search payload from the active stats + base properties,
/// pruning any filter group left empty.
fn build_query(parsed_stats: &[ParsedStat], base_properties: &[BaseProp]) -> Value {
    let mut query = json!({
        "query": {
            "status": {"option": "securable"},
            "filters": {
                "trade_filters": {"filters": {"sale_type": {"option": "any"}}},
                "type_filters": {"filters": {}},
                "misc_filters": {"filters": {}},
                "equipment_filters": {"filters": {}}
            }
        },
        "sort": {"price": "asc"}
    });

    for bp in base_properties.iter().filter(|b| b.active) {
        match bp.id.as_str() {
            "rarity" => {
                query["query"]["filters"]["type_filters"]["filters"]["rarity"] =
                    json!({"option": bp.value.to_lowercase()});
            }
            "class" => {
                if let Some(cat) = category_for(&bp.value) {
                    query["query"]["filters"]["type_filters"]["filters"]["category"] =
                        json!({"option": cat});
                }
            }
            "base" => query["query"]["type"] = json!(bp.value),
            "name" => query["query"]["name"] = json!(bp.value),
            "ilvl" => {
                query["query"]["filters"]["misc_filters"]["filters"]["ilvl"] =
                    json!({"min": int(&bp.value)});
            }
            "gemLevel" => {
                query["query"]["filters"]["misc_filters"]["filters"]["gem_level"] =
                    json!({"min": int(&bp.value)});
            }
            "quality" => {
                query["query"]["filters"]["misc_filters"]["filters"]["quality"] =
                    json!({"min": int(&bp.value)});
            }
            "sockets" => {
                query["query"]["filters"]["equipment_filters"]["filters"]["rune_sockets"] =
                    json!({"min": int(&bp.value)});
            }
            "gem_sockets" => {
                query["query"]["filters"]["misc_filters"]["filters"]["gem_sockets"] =
                    json!({"min": int(&bp.value)});
            }
            _ => {}
        }
    }

    let stat_filters: Vec<Value> = parsed_stats
        .iter()
        .filter(|s| s.active)
        .map(|s| {
            let mut value = serde_json::Map::new();
            if !s.min.is_empty() {
                if let Ok(n) = s.min.parse::<f64>() {
                    value.insert("min".into(), json!(n));
                }
            }
            if !s.max.is_empty() {
                if let Ok(n) = s.max.parse::<f64>() {
                    value.insert("max".into(), json!(n));
                }
            }
            if value.is_empty() {
                json!({"id": s.id})
            } else {
                json!({"id": s.id, "value": Value::Object(value)})
            }
        })
        .collect();
    if !stat_filters.is_empty() {
        query["query"]["stats"] = json!([{"type": "and", "filters": stat_filters}]);
    }

    // Drop filter groups that ended up empty (the API rejects empty filter objects).
    if let Some(filters) = query["query"]["filters"].as_object_mut() {
        for group in ["type_filters", "misc_filters", "equipment_filters"] {
            let empty = filters
                .get(group)
                .and_then(|g| g.get("filters"))
                .and_then(|f| f.as_object())
                .is_some_and(|f| f.is_empty());
            if empty {
                filters.remove(group);
            }
        }
    }
    query
}

fn int(s: &str) -> i64 {
    s.parse().unwrap_or(0)
}

/// Currency code → short display symbol; the reference's normalization table.
fn currency_symbol(cur: &str) -> String {
    match cur {
        "chaos" => "C",
        "divine" => "D",
        "alch" | "orb-of-alchemy" => "A",
        "annul" => "An",
        "exalt" | "exalty" | "exalted" => "E",
        "regal" => "R",
        "vaal" => "V",
        _ => return cur.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
    }
    .to_string()
}

/// Turn the fetch response into sorted, exalt-normalized listings (cheapest first).
fn normalize_listings(body: &Value, rates: &std::collections::HashMap<String, f64>) -> Vec<Listing> {
    let Some(results) = body.get("result").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    let mut prices: Vec<Listing> = Vec::new();
    for res in results {
        let Some(listing) = res.get("listing") else {
            continue;
        };
        let Some(price) = listing.get("price") else {
            continue;
        };
        let amount = price.get("amount").and_then(|a| a.as_f64());
        let currency = price
            .get("currency")
            .and_then(|c| c.as_str())
            .map(str::to_lowercase);
        let (Some(amt), Some(cur)) = (amount, currency) else {
            continue;
        };
        let exalt_val = amt * rates.get(&cur).copied().unwrap_or(0.0);
        let sym = currency_symbol(&cur);
        let mut display = format!("{} {sym}", trim_num(amt));
        if sym != "E" && sym != "D" {
            display = format!("{display} ({} E)", super::round2(exalt_val));
        }
        let age = listing
            .get("indexed")
            .and_then(|i| i.as_str())
            .map(format_age)
            .unwrap_or_default();
        prices.push(Listing {
            display,
            exalt_val,
            age,
        });
    }
    prices.sort_by(|a, b| a.exalt_val.total_cmp(&b.exalt_val));
    prices.truncate(10);
    prices
}

/// Display an amount without a trailing `.0` for whole numbers (`5`, not `5.0`).
fn trim_num(amt: f64) -> String {
    if amt.fract() == 0.0 {
        format!("{}", amt as i64)
    } else {
        format!("{}", super::round2(amt))
    }
}

/// Parse the `Retry-After` seconds from a 429 (default 60s if absent/garbage).
fn retry_after_secs(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(60)
}

/// Relative age of an ISO-8601 (`YYYY-MM-DDTHH:MM:SSZ`) timestamp: `3s` / `4m` /
/// `2h` / `5d`. Empty string if the timestamp can't be parsed.
fn format_age(indexed: &str) -> String {
    let Some(then) = parse_iso_to_unix(indexed) else {
        return String::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = (now - then).max(0);
    if diff < 60 {
        format!("{diff}s")
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86400 {
        format!("{}h", diff / 3600)
    } else {
        format!("{}d", diff / 86400)
    }
}

/// Parse `YYYY-MM-DDTHH:MM:SSZ` (UTC) to a Unix timestamp without a date crate.
fn parse_iso_to_unix(s: &str) -> Option<i64> {
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se)
}

/// Days since the Unix epoch for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[allow(clippy::too_many_arguments)]
fn result(
    name: &str,
    league: &str,
    status: PriceStatus,
    message: Option<String>,
    listings: Vec<Listing>,
    parsed_stats: Vec<ParsedStat>,
    base_properties: Vec<BaseProp>,
    snapshot: &CacheSnapshot,
) -> PriceResult {
    PriceResult {
        status,
        item: name.to_string(),
        message,
        listings,
        parsed_stats,
        base_properties,
        league: league.to_string(),
        leagues: snapshot.leagues.clone(),
    }
}
