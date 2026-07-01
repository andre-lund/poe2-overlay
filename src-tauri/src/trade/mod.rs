//! Pricing core (plan T4, ADR-0004).
//!
//! Parses PoE2 clipboard item text, then prices it: bulk/stackables via poe.ninja
//! (zero GGG quota) and gear/waystones via the official GGG trade2 API
//! (`/api/trade2/search` + `/fetch`), honoring the `X-Rate-Limit` headers to avoid
//! IP lockouts. Unlike the per-keypress Python reference (PathofTrading, GPLv3 —
//! technique reference only, ADR-0001), this lives in the persistent app so the
//! HTTP client + DNS connection pool stay warm between checks.
//!
//! Layout: [`parse`] turns clipboard text into a [`ParsedItem`]; [`stats`] maps stat
//! lines to trade2 stat ids ([`stats::StatMapper`]) and item names to base types
//! ([`stats::ItemMapper`]); [`ninja`] does the bulk path; [`gear`] builds and runs
//! the trade2 search+fetch. [`Pricing`] is the warm-client + cache state held in
//! Tauri state and drives [`Pricing::price`].

mod gear;
mod ninja;
mod parse;
mod stats;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex as AsyncMutex;

pub use parse::parse_item;

/// Offline fallback league, used only when the live league list can't be fetched.
/// The active league is normally resolved to the current challenge league from the
/// fetched list (GGG trade2 rejects a stale league in the search path), so this is
/// just a last resort — keep it pointed at the current league for fresh-install/offline.
pub const DEFAULT_LEAGUE: &str = "Runes of Aldur";

/// Browser-ish UA matching the validated reference's shape but identifying this
/// app. The trade2 API serves search/fetch to this without a session cookie.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) poe2-overlay/0.1";

/// trade2 `data/stats` / `data/items` and the league list change at most on a
/// patch; refresh daily. Exchange rates move within a league, so refresh often.
const TRADE_DATA_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const RATES_TTL: Duration = Duration::from_secs(15 * 60);

// --- Parsed item -----------------------------------------------------------

/// A single modifier line parsed off the item text, with the affix metadata the
/// query builder needs (tier for display, source = explicit/implicit/… ).
#[derive(Debug, Clone)]
pub struct ItemStat {
    pub text: String,
    pub tier: Option<u32>,
    pub source: String,
}

/// A PoE2 item parsed from clipboard text. `is_bulk` routes currency/stackables to
/// poe.ninja; everything else goes to the trade2 gear path.
#[derive(Debug, Clone)]
pub struct ParsedItem {
    pub item_class: String,
    pub rarity: String,
    pub name: String,
    pub base_type: String,
    pub is_bulk: bool,
    pub ilvl: Option<u32>,
    pub quality: Option<u32>,
    pub sockets: Option<u32>,
    pub gem_level: Option<u32>,
    pub stats: Vec<ItemStat>,
}

// --- Result types (serialized to the overlay; see ADR-0004 event contract) ---

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PriceStatus {
    Success,
    Empty,
    RateLimited,
    Error,
}

/// One market listing, normalized to an exalt-equivalent value for sorting.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// Human display, e.g. `"5 D"` or `"12 C (1.55 E)"`.
    pub display: String,
    pub exalt_val: f64,
    /// Relative age of the listing, e.g. `"3h"`, or `"poe.ninja avg"` for bulk.
    pub age: String,
}

/// A parsed stat line resolved to a trade2 stat id, with the value used to seed the
/// search filter. Sent to the overlay so T5 can render per-stat toggles + requery
/// (round-trips back into the `requery` command, hence `Deserialize`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedStat {
    pub id: String,
    pub text: String,
    pub tier: Option<u32>,
    pub value: Option<f64>,
    /// Lower bound seeded into the query (empty string = unset).
    pub min: String,
    pub max: String,
    pub active: bool,
}

/// A base property (class / rarity / base type / name / ilvl / …) that can be
/// toggled into the trade2 query. `active` mirrors the reference's defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseProp {
    pub id: String,
    pub text: String,
    /// String form for display + query construction; ints arrive as their decimal.
    pub value: String,
    pub active: bool,
}

/// The pricing outcome handed to the overlay (`price-check-result`). `parsed_stats`
/// + `base_properties` carry the toggle state forward to the T5 requery UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceResult {
    pub status: PriceStatus,
    pub item: String,
    /// Status / error text shown when there are no listings.
    pub message: Option<String>,
    pub listings: Vec<Listing>,
    pub parsed_stats: Vec<ParsedStat>,
    pub base_properties: Vec<BaseProp>,
    pub league: String,
    pub leagues: Vec<String>,
}

impl PriceResult {
    /// Result for clipboard text that did not parse as a PoE2 item.
    pub fn invalid() -> Self {
        PriceResult::message(
            "No item",
            DEFAULT_LEAGUE,
            PriceStatus::Error,
            "No PoE2 item under the cursor — hover an item, then press Ctrl+Alt+D.",
            Vec::new(),
        )
    }

    fn message(
        item: &str,
        league: &str,
        status: PriceStatus,
        message: impl Into<String>,
        leagues: Vec<String>,
    ) -> Self {
        PriceResult {
            status,
            item: item.to_string(),
            message: Some(message.into()),
            listings: Vec::new(),
            parsed_stats: Vec::new(),
            base_properties: Vec::new(),
            league: league.to_string(),
            leagues,
        }
    }
}

/// Display name for the result header: rares show `Name (Base Type)`.
pub fn display_name(item: &ParsedItem) -> String {
    if item.rarity == "Rare" && item.name != item.base_type && !item.base_type.is_empty() {
        format!("{} ({})", item.name, item.base_type)
    } else {
        item.name.clone()
    }
}

/// A successful bulk (poe.ninja) result — one listing, no toggleable filters.
fn bulk_result(
    item: &ParsedItem,
    league: String,
    leagues: Vec<String>,
    listing: Listing,
) -> PriceResult {
    PriceResult {
        status: PriceStatus::Success,
        item: display_name(item),
        message: None,
        listings: vec![listing],
        parsed_stats: Vec::new(),
        base_properties: Vec::new(),
        league,
        leagues,
    }
}

// --- Rate-limit lockout ----------------------------------------------------

/// IP rate-limit guard. The trade2 API returns `X-Rate-Limit-Ip[-State]` headers;
/// when a window is one request from its cap we self-impose a wait so we never trip
/// GGG's IP ban (the one un-recoverable failure mode — ADR-0004).
#[derive(Default)]
struct RateLimit {
    until: Option<Instant>,
}

/// Never lock out longer than an hour, no matter what a (possibly hostile) header says.
const MAX_LOCKOUT: Duration = Duration::from_secs(3600);

impl RateLimit {
    /// Remaining lockout in whole seconds, or `None` if clear.
    fn wait_secs(&self) -> Option<u64> {
        let until = self.until?;
        let now = Instant::now();
        if until > now {
            Some(until.saturating_duration_since(now).as_secs().max(1))
        } else {
            None
        }
    }

    /// Extend the lockout to at least `when` (never shortens an existing, longer one).
    fn extend_to(&mut self, when: Instant) {
        if self.until.is_none_or(|u| when > u) {
            self.until = Some(when);
        }
    }

    /// Arm the lockout to at least `secs` from now — used for a 429 `Retry-After`,
    /// which is GGG's real penalty (commonly 60s+), far longer than the `window/limit`
    /// estimate `apply_headers` derives. Without this a 429 would only self-throttle
    /// ~1s and the next check could fire straight back into an active penalty, the IP-
    /// ban path ADR-0004 exists to prevent.
    fn arm_secs(&mut self, secs: u64) {
        let secs = secs.min(MAX_LOCKOUT.as_secs());
        if let Some(when) = Instant::now().checked_add(Duration::from_secs(secs.max(1))) {
            self.extend_to(when);
        }
    }

    /// Parse the IP rate-limit headers and arm a lockout if any window is within one
    /// request of its limit, or if GGG reports an already-active restriction. Header
    /// shape: `X-Rate-Limit-Ip: limit:window:penalty,…` paired positionally with
    /// `X-Rate-Limit-Ip-State: count:window:active,…` (field 2 = seconds of a
    /// restriction already in force). Hostile/garbage values can never panic here.
    fn apply_headers(&mut self, headers: &reqwest::header::HeaderMap) {
        let state = headers
            .get("X-Rate-Limit-Ip-State")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let rules = headers
            .get("X-Rate-Limit-Ip")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if state.is_empty() || rules.is_empty() {
            return;
        }
        let mut max_wait = 0.0_f64;
        for (s_part, r_part) in state.split(',').zip(rules.split(',')) {
            let mut s = s_part.split(':');
            let count: f64 = match s.next().and_then(|n| n.trim().parse().ok()) {
                Some(n) => n,
                None => continue,
            };
            // Field 2 of the state part: seconds of a restriction already imposed.
            let _state_window = s.next();
            if let Some(active) = s.next().and_then(|n| n.trim().parse::<f64>().ok()) {
                if active.is_finite() && active > max_wait {
                    max_wait = active;
                }
            }
            let mut r = r_part.split(':');
            let limit: f64 = match r.next().and_then(|n| n.trim().parse().ok()) {
                Some(n) => n,
                None => continue,
            };
            let window: f64 = match r.next().and_then(|n| n.trim().parse().ok()) {
                Some(n) => n,
                None => continue,
            };
            if limit - count <= 1.0 && limit > 0.0 {
                let wait = window / limit;
                if wait.is_finite() && wait > max_wait {
                    max_wait = wait;
                }
            }
        }
        if max_wait > 0.0 {
            let max_wait = max_wait.min(MAX_LOCKOUT.as_secs_f64());
            if let Some(when) = Instant::now().checked_add(Duration::from_secs_f64(max_wait)) {
                self.extend_to(when);
            }
        }
    }
}

// --- Caches ----------------------------------------------------------------

/// trade2 `data/stats` entry — only id + display text are used for mapping.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct StatEntry {
    pub id: String,
    #[serde(default)]
    pub text: String,
}

#[derive(serde::Deserialize)]
struct StatCategory {
    #[serde(default)]
    entries: Vec<StatEntry>,
}

#[derive(serde::Deserialize)]
struct StatsResponse {
    #[serde(default)]
    result: Vec<StatCategory>,
}

#[derive(serde::Deserialize)]
struct ItemEntry {
    #[serde(default)]
    #[serde(rename = "type")]
    type_: Option<String>,
}

#[derive(serde::Deserialize)]
struct ItemCategory {
    #[serde(default)]
    entries: Vec<ItemEntry>,
}

#[derive(serde::Deserialize)]
struct ItemsResponse {
    #[serde(default)]
    result: Vec<ItemCategory>,
}

/// Lazily-fetched, daily-stale trade2 reference data + exchange rates. Held behind a
/// single async mutex; price checks are serialized by the hotkey so contention is nil.
struct Caches {
    stats: Option<(Instant, Arc<Vec<StatEntry>>)>,
    items: Option<(Instant, Arc<Vec<String>>)>,
    leagues: Option<(Instant, Vec<String>)>,
    rates: HashMap<String, f64>,
    rates_at: Option<Instant>,
    /// League the cached `rates` were fetched for — exchange rates are league-specific,
    /// so switching league (T5 selector) must refetch even within the TTL.
    rates_league: Option<String>,
}

/// League-agnostic fallback exchange rates (exalt-equivalents) — used at startup and
/// when a league-switch refetch fails (so we never serve another league's ratios).
fn seeded_rates() -> HashMap<String, f64> {
    HashMap::from([
        ("whetstone".into(), 2.21),
        ("exalted".into(), 1.0),
        ("exalt".into(), 1.0),
        ("divine".into(), 193.0),
        ("vaal".into(), 4.45),
        ("chaos".into(), 7.70),
        ("alch".into(), 0.32),
        ("regal".into(), 0.38),
        ("mirror".into(), 1_640_154.0),
    ])
}

impl Default for Caches {
    fn default() -> Self {
        Caches {
            stats: None,
            items: None,
            leagues: None,
            // Seed with the reference's static fallback so bulk pricing degrades
            // gracefully when poe.ninja is unreachable.
            rates: seeded_rates(),
            rates_at: None,
            rates_league: None,
        }
    }
}

/// Cheap clone handed to the gear/ninja paths so the async cache mutex is not held
/// across the trade2 round-trip. Reference vecs are shared via `Arc`.
pub(crate) struct CacheSnapshot {
    pub stats: Arc<Vec<StatEntry>>,
    pub items: Arc<Vec<String>>,
    pub rates: HashMap<String, f64>,
    pub leagues: Vec<String>,
    /// The league actually queried — the user override if it is a current league,
    /// otherwise the first fetched league (current challenge league).
    pub league: String,
}

fn fresh<T>(slot: &Option<(Instant, T)>, ttl: Duration) -> bool {
    slot.as_ref()
        .is_some_and(|(at, _)| at.elapsed() < ttl)
}

// --- Pricing state ---------------------------------------------------------

/// Warm-client pricing state, stored in Tauri state via `app.manage(Pricing::new())`
/// and shared by every price check (ADR-0004).
pub struct Pricing {
    client: reqwest::Client,
    cache: AsyncMutex<Caches>,
    rate: Mutex<RateLimit>,
    /// User-selected league override (T5 selector sets it); `None` = use the current
    /// challenge league resolved from the fetched list.
    league: Mutex<Option<String>>,
    /// The last item priced, so the T5 requery can re-price it with edited filters /
    /// a new league without the frontend round-tripping the whole item.
    last_item: Mutex<Option<ParsedItem>>,
}

impl Pricing {
    pub fn new() -> Self {
        // `build()` only configures the pool; no runtime needed until `send().await`.
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(12))
            .build()
            .expect("reqwest client builds with rustls");
        Pricing {
            client,
            cache: AsyncMutex::new(Caches::default()),
            rate: Mutex::new(RateLimit::default()),
            league: Mutex::new(None),
            last_item: Mutex::new(None),
        }
    }

    /// Override the active league (T5 league selector). `None` reverts to auto
    /// (current challenge league). Persists to the next hotkey check too.
    pub fn set_league(&self, league: Option<String>) {
        *self.league.lock().unwrap_or_else(|e| e.into_inner()) = league;
    }

    /// Current IP-rate-limit lockout in seconds, or `None` if clear. Poison-recovering
    /// so a stray panic can never brick pricing (ADR-0004 never-panics guarantee).
    fn check_lockout(&self) -> Option<u64> {
        self.rate.lock().unwrap_or_else(|e| e.into_inner()).wait_secs()
    }

    /// Price a parsed item. Bulk currency/stackables resolve via poe.ninja (no GGG
    /// quota); gear and waystones go through the trade2 search+fetch path, gated by the
    /// IP-rate-limit lockout. Never panics — failures become an `Empty`/`Error`/
    /// `RateLimited` result the overlay renders as text.
    pub async fn price(&self, item: &ParsedItem) -> PriceResult {
        // Remember the item so T5's requery can re-price it with edited filters.
        *self.last_item.lock().unwrap_or_else(|e| e.into_inner()) = Some(item.clone());

        let snapshot = self.ensure_caches().await;
        let league = snapshot.league.clone();

        // Bulk intercept: stackables priced from poe.ninja (no GGG quota, not lockout-
        // gated). Waystones are bulk-classed but priced as gear, so they fall through.
        if item.is_bulk && !item.name.to_lowercase().contains("waystone") {
            if let Some(listing) =
                ninja::price_bulk(&self.client, &league, item, &snapshot.rates).await
            {
                return bulk_result(item, league, snapshot.leagues, listing);
            }
            // poe.ninja has no price for it — fall through to the trade2 auction path.
        }

        // Gear hits the GGG trade2 search/fetch — respect the IP lockout so we never
        // trip a ban.
        if let Some(wait) = self.check_lockout() {
            return PriceResult::message(
                &display_name(item),
                &league,
                PriceStatus::RateLimited,
                format!("Rate limit approaching — wait {wait}s"),
                snapshot.leagues,
            );
        }

        gear::price_gear(&self.client, &league, item, &snapshot, &self.rate).await
    }

    /// Re-price the last-checked item with user-edited filters and/or a new league (the
    /// T5 toggles + league selector). Sets the league override (so a later hotkey check
    /// uses it too), then re-runs: bulk via poe.ninja for the new league, gear via the
    /// trade2 query built from the *edited* stats/base. Honors the IP lockout.
    pub async fn requery(
        &self,
        league: String,
        parsed_stats: Vec<ParsedStat>,
        base_properties: Vec<BaseProp>,
    ) -> PriceResult {
        self.set_league(Some(league));
        let snapshot = self.ensure_caches().await;
        let lg = snapshot.league.clone();

        let Some(item) = self
            .last_item
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        else {
            return PriceResult::message(
                "No item",
                &lg,
                PriceStatus::Error,
                "Nothing to requery — price-check an item first",
                snapshot.leagues,
            );
        };

        if item.is_bulk && !item.name.to_lowercase().contains("waystone") {
            if let Some(listing) =
                ninja::price_bulk(&self.client, &lg, &item, &snapshot.rates).await
            {
                return bulk_result(&item, lg, snapshot.leagues, listing);
            }
        }

        if let Some(wait) = self.check_lockout() {
            // Thread the edited filters back (NOT PriceResult::message, which empties
            // them) so the overlay keeps the user's toggles/min-max to re-requery after
            // the wait — matching the reference's lockout path (backend.py).
            return PriceResult {
                status: PriceStatus::RateLimited,
                item: display_name(&item),
                message: Some(format!("Rate limit approaching — wait {wait}s")),
                listings: Vec::new(),
                parsed_stats,
                base_properties,
                league: lg,
                leagues: snapshot.leagues,
            };
        }

        gear::run_gear_query(
            &self.client,
            &lg,
            display_name(&item),
            parsed_stats,
            base_properties,
            &snapshot,
            &self.rate,
        )
        .await
    }

    /// Ensure the trade2 reference data + exchange rates are loaded and fresh, then
    /// hand back a cheap snapshot. All fetches are best-effort: a failure leaves the
    /// prior (or empty/seeded) value so pricing still runs, just less precisely. The
    /// active league is resolved here (leagues are fetched first, since the exchange
    /// rates and the search path both need a *valid* league).
    async fn ensure_caches(&self) -> CacheSnapshot {
        let mut c = self.cache.lock().await;

        if !fresh(&c.stats, TRADE_DATA_TTL) {
            if let Some(stats) = fetch_stats(&self.client).await {
                c.stats = Some((Instant::now(), Arc::new(stats)));
            } else if c.stats.is_none() {
                c.stats = Some((Instant::now(), Arc::new(Vec::new())));
            }
        }
        if !fresh(&c.items, TRADE_DATA_TTL) {
            if let Some(items) = fetch_items(&self.client).await {
                c.items = Some((Instant::now(), Arc::new(items)));
            } else if c.items.is_none() {
                c.items = Some((Instant::now(), Arc::new(Vec::new())));
            }
        }
        if !fresh(&c.leagues, TRADE_DATA_TTL) {
            if let Some(leagues) = fetch_leagues(&self.client).await {
                c.leagues = Some((Instant::now(), leagues));
            } else if c.leagues.is_none() {
                c.leagues = Some((Instant::now(), default_leagues()));
            }
        }

        let leagues = c
            .leagues
            .as_ref()
            .map(|(_, l)| l.clone())
            .unwrap_or_else(default_leagues);
        let league = self.resolve_league(&leagues);

        // Refetch exchange rates when stale OR when the league changed (rates are
        // league-specific). On success, replace wholesale — merging would leave the
        // previous league's values for currencies absent from the new overview.
        let rates_stale = c.rates_at.is_none_or(|at| at.elapsed() >= RATES_TTL);
        let league_changed = c.rates_league.as_deref() != Some(league.as_str());
        if rates_stale || league_changed {
            if let Some(rates) = ninja::fetch_exchange_rates(&self.client, &league).await {
                c.rates = rates;
                c.rates_at = Some(Instant::now());
                c.rates_league = Some(league.clone());
            } else if league_changed {
                // Refetch failed for a *different* league — never serve the prior
                // league's ratios as this one. Drop to neutral seeds and leave rates_at
                // unset so the next check retries.
                c.rates = seeded_rates();
                c.rates_at = None;
                c.rates_league = Some(league.clone());
            }
        }

        CacheSnapshot {
            stats: c.stats.as_ref().map(|(_, s)| s.clone()).unwrap_or_default(),
            items: c.items.as_ref().map(|(_, i)| i.clone()).unwrap_or_default(),
            rates: c.rates.clone(),
            leagues,
            league,
        }
    }

    /// The league to query: the user override if it is one of the current leagues,
    /// else the first fetched league (the current challenge league), else the fallback.
    fn resolve_league(&self, leagues: &[String]) -> String {
        let override_ = self.league.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match override_ {
            Some(l) if leagues.contains(&l) => l,
            _ => leagues
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_LEAGUE.to_string()),
        }
    }
}

/// Round to two decimal places, matching the reference's `round(x, 2)` display
/// (trailing zeros dropped by `f64`'s `Display`).
pub(crate) fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

impl Default for Pricing {
    fn default() -> Self {
        Self::new()
    }
}

fn default_leagues() -> Vec<String> {
    vec![
        DEFAULT_LEAGUE.to_string(),
        format!("HC {DEFAULT_LEAGUE}"),
        "Standard".to_string(),
        "Hardcore".to_string(),
    ]
}

// --- Reference-data fetches ------------------------------------------------

async fn fetch_stats(client: &reqwest::Client) -> Option<Vec<StatEntry>> {
    let resp = client
        .get("https://www.pathofexile.com/api/trade2/data/stats")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: StatsResponse = resp.json().await.ok()?;
    Some(
        body.result
            .into_iter()
            .flat_map(|c| c.entries)
            .collect(),
    )
}

async fn fetch_items(client: &reqwest::Client) -> Option<Vec<String>> {
    let resp = client
        .get("https://www.pathofexile.com/api/trade2/data/items")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: ItemsResponse = resp.json().await.ok()?;
    let mut types: Vec<String> = body
        .result
        .into_iter()
        .flat_map(|c| c.entries)
        .filter_map(|e| e.type_)
        .filter(|t| !t.is_empty())
        .collect();
    // Longest-first so `get_base_name` prefers the most specific substring match.
    types.sort_by_key(|b| std::cmp::Reverse(b.len()));
    Some(types)
}

async fn fetch_leagues(client: &reqwest::Client) -> Option<Vec<String>> {
    // Prefer poe.ninja for clean economy-league names.
    if let Ok(resp) = client
        .get("https://poe.ninja/poe2/api/data/index-state")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<serde_json::Value>().await {
                let leagues: Vec<String> = v
                    .get("economyLeagues")
                    .and_then(|l| l.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                if !leagues.is_empty() {
                    return Some(leagues);
                }
            }
        }
    }
    // Fallback: GGG trade2 league list (poe2 realm only).
    let resp = client
        .get("https://www.pathofexile.com/api/trade2/data/leagues")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    let leagues: Vec<String> = v
        .get("result")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|l| l.get("realm").and_then(|r| r.as_str()) == Some("poe2"))
                .filter_map(|l| l.get("id").and_then(|i| i.as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    (!leagues.is_empty()).then_some(leagues)
}

#[cfg(test)]
mod net_tests {
    //! Live end-to-end smoke tests against poe.ninja + the GGG trade2 API. `#[ignore]`d
    //! so normal `cargo test` stays offline; run with `cargo test -- --ignored`.
    use super::*;

    fn currency(name: &str) -> ParsedItem {
        parse_item(&format!(
            "Item Class: Stackable Currency\nRarity: Currency\n{name}\n--------\nStack Size: 1/10"
        ))
        .unwrap()
    }

    #[test]
    #[ignore = "hits live poe.ninja"]
    fn smoke_bulk_divine_and_chaos() {
        let p = Pricing::new();
        let div = tauri::async_runtime::block_on(p.price(&currency("Divine Orb")));
        eprintln!("Divine Orb → {:?} {:?}", div.status, div.listings);
        assert_eq!(div.status, PriceStatus::Success);
        assert_eq!(div.listings[0].display, "1 D");

        let chaos = tauri::async_runtime::block_on(p.price(&currency("Chaos Orb")));
        eprintln!("Chaos Orb → {:?} {:?}", chaos.status, chaos.listings);
        assert_eq!(chaos.status, PriceStatus::Success);
        assert!(chaos.listings[0].exalt_val > 0.0);
        assert!(chaos.listings[0].display.ends_with('E'));
    }

    #[test]
    #[ignore = "hits the live GGG trade2 search+fetch API (uses rate-limit budget)"]
    fn smoke_gear_rare_body_armour() {
        // "Requires: Level" is metadata (skipped); each resistance maps to its own
        // per-element pseudo, life into pseudo total life.
        let text = "Item Class: Body Armours\n\
            Rarity: Rare\n\
            Doom Shell\n\
            Vaal Regalia\n\
            --------\n\
            Energy Shield: 200\n\
            --------\n\
            Requirements:\n\
            Requires: Level 65, 159 Int\n\
            --------\n\
            Item Level: 82\n\
            --------\n\
            +89 to maximum Life\n\
            +45% to Fire Resistance\n\
            +30% to Cold Resistance";
        let item = parse_item(text).unwrap();
        let p = Pricing::new();
        let r = tauri::async_runtime::block_on(p.price(&item));
        eprintln!(
            "gear → status={:?} msg={:?} listings={} stats={:?}",
            r.status,
            r.message,
            r.listings.len(),
            r.parsed_stats.iter().map(|s| (&s.id, &s.min)).collect::<Vec<_>>()
        );
        // The round-trip + serde must succeed (no Error status); a real query returns
        // Success or Empty depending on current listings.
        assert!(matches!(r.status, PriceStatus::Success | PriceStatus::Empty));
        // Life + each resistance mapped to its own per-element pseudo (no combined total).
        assert!(r
            .parsed_stats
            .iter()
            .any(|s| s.id == "pseudo.pseudo_total_fire_resistance"));
        assert!(r
            .parsed_stats
            .iter()
            .any(|s| s.id == "pseudo.pseudo_total_cold_resistance"));
        assert!(r.parsed_stats.iter().any(|s| s.id == "pseudo.pseudo_total_life"));
    }

    #[test]
    #[ignore = "hits the live GGG trade2 search+fetch API (uses rate-limit budget)"]
    fn smoke_requery_with_edited_filters() {
        let text = "Item Class: Body Armours\n\
            Rarity: Rare\n\
            Doom Shell\n\
            Vaal Regalia\n\
            --------\n\
            Item Level: 82\n\
            --------\n\
            +89 to maximum Life\n\
            +45% to Fire Resistance\n\
            +30% to Cold Resistance";
        let item = parse_item(text).unwrap();
        let p = Pricing::new();
        let first = tauri::async_runtime::block_on(p.price(&item));
        assert!(matches!(first.status, PriceStatus::Success | PriceStatus::Empty));

        // Requery with every stat filter toggled off (base-category only) on the
        // resolved league — exercises run_gear_query from edited filters + last_item.
        let mut stats = first.parsed_stats.clone();
        for s in &mut stats {
            s.active = false;
        }
        let r = tauri::async_runtime::block_on(p.requery(
            first.league.clone(),
            stats,
            first.base_properties.clone(),
        ));
        eprintln!("requery → status={:?} listings={}", r.status, r.listings.len());
        assert!(matches!(r.status, PriceStatus::Success | PriceStatus::Empty));
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    fn headers(rules: &str, state: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("X-Rate-Limit-Ip", HeaderValue::from_str(rules).unwrap());
        h.insert("X-Rate-Limit-Ip-State", HeaderValue::from_str(state).unwrap());
        h
    }

    #[test]
    fn arm_secs_extends_but_never_shortens() {
        let mut r = RateLimit::default();
        r.arm_secs(60);
        assert!((59..=60).contains(&r.wait_secs().unwrap()));
        r.arm_secs(5); // shorter — must not replace the live 60s lockout
        assert!(r.wait_secs().unwrap() >= 59);
    }

    #[test]
    fn hostile_header_clamps_and_never_panics() {
        let mut r = RateLimit::default();
        // Absurd window with limit 1 → window/limit is enormous; must clamp, not panic.
        r.apply_headers(&headers("1:99999999999999999999:0", "5:10:0"));
        assert!((3599..=3600).contains(&r.wait_secs().unwrap()));
    }

    #[test]
    fn active_restriction_field_arms_lockout() {
        let mut r = RateLimit::default();
        // State field 2 = 120s of restriction already in force (limit not near cap).
        r.apply_headers(&headers("8:10:0", "0:10:120"));
        assert!((119..=120).contains(&r.wait_secs().unwrap()));
    }

    #[test]
    fn clear_when_under_limit() {
        let mut r = RateLimit::default();
        r.apply_headers(&headers("8:10:0", "2:10:0"));
        assert_eq!(r.wait_secs(), None);
    }
}
