---
status: proposed
---

# 0008 — Support localized game clients by translating the item copy to English up front

**Implemented by:** _pending_
**Relates to:** ADR-0004 (pricing core — unchanged, still English-only), ADR-0005 (waystone rules — unchanged)

A copied item from a non-English game client (ru, de, fr, es, pt, ja, ko) is
translated into the English clipboard format **before** `parse_item`, so the parser,
the stat mapper, the poe.ninja path and the waystone danger rules keep working on
English text only. The UI language is a persisted setting (`auto` = follow the game
client, or one of the nine trade2 languages incl. Thai), and only display texts are
localized — every query is still built from English names and language-neutral ids.

## Context

The game copies item text in the client's language (`Класс предмета: Кольца`,
`Gegenstandsklasse: Ringe`, …). Every stage of the pipeline matches English
literals (`Item Class:`, `Rarity: Rare`, stat templates, danger keywords), so a
localized copy was rejected as "No item". Localizing each stage would fork the
parser, the stat mapper, poe.ninja name lookup and the danger ruleset per language.
GGG serves the same trade2 reference data (`data/stats`, `items`, `static`,
`filters`) per language on `<host>.pathofexile.com`, keyed by the same ids.

## Decision

- **One translation step at the entry** (`trade/i18n.rs`, `LangDict` per language,
  built daily from `www.` + the language's host):
  - stat lines: `data/stats` ids are language-neutral — the localized template keys
    the English one, the copied numbers are substituted back (literal numbers baked
    into a template are told apart from `#` slots);
  - currency/bulk names: `data/static` entries share ids;
  - uniques/base types: `data/items` lists entries in the same order per category;
    uniques pair 1:1, base lists are aligned with the unique pairs as anchors;
  - item class: the plural in-game class is matched by word stem against the
    singular `category` filter labels; failing that, the class of the translated
    base type from RePoE (`repoe-fork.github.io/poe2`, fetched only when needed);
  - fixed copy wording (header keys, rarities, affix-header kinds, flags, the
    unmet-requirements notice) per language from Exiled-Exchange-2's
    `client_strings` (MIT).
- **Language detection** from the item-class header line; English copies pass
  through untouched. Thai: its trade data keeps English item names, so the client is
  treated as English for copies and only the UI is translated.
- **UI language** (`settings.rs`, `~/.config/io.olund.poe2overlay/settings.json`):
  card labels, backend messages, price-sheet names/categories and waystone danger
  texts follow it. The card shows the copy's own wording (mods, class, base, name)
  only when the UI language equals the copy's; otherwise English.

## Consequences

- Localized players get prices and danger checks with no change to the pricing or
  danger logic; English players see no behavior change.
- First localized copy (per language, per day) costs ~5 MB of trade2 data fetches;
  an unresolvable item class costs one RePoE fetch (~3 MB).
- Multi-line stats (a few dozen uniques' mods) are not translated and are left out
  of the search, as an unmapped English line would be.
- UI translations other than Russian are best-effort; corrections welcome.
- Deferred: Traditional Chinese (`pathofexile.tw` is a separate realm).
