//! Item parsing + pricing.
//!
//! Parses PoE2 clipboard item text, then prices it: bulk/stackables via
//! poe.ninja (zero GGG quota) and gear/waystones via the official GGG trade2
//! API (`/api/trade2/search` + `/fetch`), honoring the `X-Rate-Limit` headers
//! to avoid IP lockouts. Unlike the per-keypress Python reference, this runs in
//! the persistent app so the HTTP client + DNS stay warm between checks.

/// Parse PoE2 clipboard item text into a structured item.
pub fn parse_item(_text: &str) {
    // TODO(T4): port the parser shape from the references (Exiled-Exchange-2
    // item parser; PathofTrading backend.py parse_item).
    unimplemented!("item parsing — plan T4")
}

/// Price a parsed item via poe.ninja (bulk) or the GGG trade2 API (gear).
pub fn price_item() {
    // TODO(T4): requires `reqwest`. Reference: ExileWatch trade2 query builder.
    unimplemented!("pricing — plan T4")
}
