//! Russian game client → English item text.
//!
//! The parser, stat mapper, poe.ninja path and waystone danger rules all speak the
//! English clipboard format. Rather than localize each of them, a Russian copy is
//! translated to the English shape up front, using the official trade2 reference data
//! fetched from both `www.` and `ru.pathofexile.com`:
//!
//! - stat lines: `data/stats` ids are language-neutral, so a Russian template
//!   (`# к максимуму здоровья`) keys the English one (`# to maximum Life`) by id and
//!   the copied numbers are substituted back in;
//! - currency / bulk names: `data/static` entries share ids across languages;
//! - uniques and base types: `data/items` lists the same entries in the same order per
//!   category; uniques pair 1:1, base lists are aligned using the unique pairs as
//!   anchors (the lists can differ by an entry);
//! - item classes: the in-game (plural) class name is matched by word stems against
//!   the trade2 `category` filter labels (singular), whose ids are language-neutral.
//!
//! Anything that can't be translated is passed through unchanged, so a partial miss
//! degrades to an unmapped stat line rather than a failed check.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

static RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // `104(101-110)` / `147(150)` / `-5(-10--1)` → keep the rolled value only.
    Regex::new(r"([+-]?\d+(?:[.,]\d+)?)\([+-]?\d+(?:[.,]\d+)?(?:-[+-]?\d+(?:[.,]\d+)?)?\)").unwrap()
});
static NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[+-]?\d+(?:[.,]\d+)?").unwrap());
/// A template slot: `#` placeholder or a literal number baked into the stat text.
static SLOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[+-]?(?:#|\d+(?:[.,]\d+)?)").unwrap());
/// Copy-only trailing markers the English parser understands as-is.
static MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s+\((implicit|rune|enchant|crafted|fractured|desecrated|augmented)\)$").unwrap()
});
static TRAILING_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\([^()]*\)$").unwrap());
/// `(Уровень: 3)` / `(Stufe: 3)` / `(Rang : 3)` — the affix tier in any language.
static TIER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\([^():：]+\s*[:：]\s*(\d+)\)").unwrap());

/// A game client language trade2 serves (`<host>.pathofexile.com`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ru,
    De,
    Fr,
    Es,
    Pt,
    Ja,
    Ko,
    Th,
}

impl Lang {
    pub const ALL: [Lang; 9] = [
        Lang::En,
        Lang::Ru,
        Lang::De,
        Lang::Fr,
        Lang::Es,
        Lang::Pt,
        Lang::Ja,
        Lang::Ko,
        Lang::Th,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::Pt => "pt",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Th => "th",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let code = code.to_lowercase();
        Lang::ALL.into_iter().find(|l| code.starts_with(l.code()))
    }

    /// The trade site / API subdomain for this language.
    pub fn host(self) -> &'static str {
        match self {
            Lang::En => "www",
            Lang::Ru => "ru",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::Pt => "br",
            Lang::Ja => "jp",
            Lang::Ko => "kr",
            Lang::Th => "th",
        }
    }

    /// Client strings of a localized item copy; `None` for languages whose client
    /// copies items in English (English itself, Thai — its trade data keeps English
    /// item names).
    pub fn strings(self) -> Option<&'static ClientStrings> {
        CLIENT_STRINGS
            .iter()
            .find(|(l, _)| *l == self)
            .map(|(_, s)| s)
    }
}

/// The fixed wording of a localized item copy. Header keys, rarities, affix-header
/// kinds and flags come from Exiled-Exchange-2's per-language `client_strings`
/// (MIT, github.com/Kvan7/Exiled-Exchange-2); negations and class stems are ours.
pub struct ClientStrings {
    pub item_class: &'static str,
    rarity: &'static str,
    item_level: &'static str,
    requires: &'static str,
    stack_size: &'static str,
    quality: &'static str,
    sockets: &'static str,
    gem_level: &'static str,
    grants_skill: &'static str,
    /// Normal, Magic, Rare, Unique, Currency, Gem, Divination Card, Quest.
    rarities: [&'static str; 8],
    /// `{ … }` affix-header kind → English `… Modifier`; longest first.
    modifiers: &'static [(&'static str, &'static str)],
    /// Whole-line flags (Corrupted, …); longest first.
    flags: &'static [(&'static str, &'static str)],
    /// Prefix of the unmet-requirements notice (see `parse::CANNOT_USE`).
    cannot_use: &'static str,
    unscalable: &'static str,
    /// Decrease wording → the increase wording the stats table carries.
    negations: &'static [(&'static str, &'static str)],
    /// Classes with no trade2 category label to match: lowercase stem → English class.
    class_stems: &'static [(&'static str, &'static str)],
}

const EN_RARITIES: [&str; 8] = [
    "Normal",
    "Magic",
    "Rare",
    "Unique",
    "Currency",
    "Gem",
    "Divination Card",
    "Quest",
];

const CLIENT_STRINGS: &[(Lang, ClientStrings)] = &[
    (
        Lang::Ru,
        ClientStrings {
            item_class: "Класс предмета:",
            rarity: "Редкость:",
            item_level: "Уровень предмета:",
            requires: "Требуется:",
            stack_size: "Размер стопки:",
            quality: "Качество:",
            sockets: "Гнезда:",
            gem_level: "Уровень:",
            grants_skill: "Дарует умение:",
            rarities: [
                "Обычный",
                "Волшебный",
                "Редкий",
                "Уникальный",
                "Валюта",
                "Камень",
                "Гадальная карта",
                "Задание",
            ],
            modifiers: &[
                ("Улучшение от осквернения", "Corrupted Implicit Modifier"),
                ("Уникальное свойство ваал", "Unique Modifier"),
                ("Уникальное свойство", "Unique Modifier"),
                ("Собственное свойство", "Implicit Modifier"),
                ("Префикс", "Prefix Modifier"),
                ("Суффикс", "Suffix Modifier"),
                ("Улучшение", "Enchant Modifier"),
                ("Ремесленное", "Crafted Modifier"),
                ("Расколотое", "Fractured Modifier"),
                ("Очернённое", "Desecrated Modifier"),
            ],
            flags: &[
                ("Дважды осквернено", "Double Corrupted"),
                ("Осквернено", "Corrupted"),
                ("Отражено", "Mirrored"),
                ("Освящено", "Sanctified"),
                ("Неизменяемый", "Unmodifiable"),
            ],
            cannot_use: "Вы не можете использовать этот предмет",
            unscalable: " — Неизменяемое значение",
            negations: &[
                ("снижение", "повышение"),
                ("снижение", "увеличение"),
                ("уменьшение", "увеличение"),
                ("уменьшение", "повышение"),
                ("меньше", "больше"),
            ],
            class_stems: &[
                ("неогран", "Uncut Skill Gem"),
                ("сущност", "Essence"),
                ("эссенц", "Essence"),
                ("валют", "Stackable Currency"),
            ],
        },
    ),
    (
        Lang::De,
        ClientStrings {
            item_class: "Gegenstandsklasse:",
            rarity: "Seltenheit:",
            item_level: "Gegenstandsstufe:",
            requires: "Erfordert:",
            stack_size: "Stapelgröße:",
            quality: "Qualität:",
            sockets: "Fassungen:",
            gem_level: "Stufe:",
            grants_skill: "Gewährt Fertigkeit:",
            rarities: [
                "Normal",
                "Magisch",
                "Selten",
                "Einzigartig",
                "Währung",
                "Gemme",
                "Weissagungskarte",
                "Quest",
            ],
            modifiers: &[
                ("Einzigartiger Vaal-Modifikator", "Unique Modifier"),
                ("Einzigartiger Modifikator", "Unique Modifier"),
                ("Impliziter Modifikator", "Implicit Modifier"),
                ("Verderbnis-Verstärkung", "Corrupted Implicit Modifier"),
                ("Präfix-Modifikator", "Prefix Modifier"),
                ("Suffix-Modifikator", "Suffix Modifier"),
                ("Verstärkung", "Enchant Modifier"),
                ("Angefertigt", "Crafted Modifier"),
                ("Brüchig", "Fractured Modifier"),
                ("Entweiht", "Desecrated Modifier"),
            ],
            flags: &[
                ("Doppelt verderbt", "Double Corrupted"),
                ("Verderbt", "Corrupted"),
                ("Gespiegelt", "Mirrored"),
                ("Geheiligt", "Sanctified"),
                ("Nicht modifizierbar", "Unmodifiable"),
            ],
            cannot_use: "Du kannst diesen Gegenstand nicht benutzen",
            unscalable: " — Nicht skalierbarer Wert",
            negations: &[("verringert", "erhöht"), ("weniger", "mehr")],
            class_stems: &[
                ("ungeschliffen", "Uncut Skill Gem"),
                ("essenz", "Essence"),
                ("währung", "Stackable Currency"),
            ],
        },
    ),
    (
        Lang::Fr,
        ClientStrings {
            item_class: "Classe d'objet:",
            rarity: "Rareté:",
            item_level: "Niveau de l'objet:",
            requires: "Prérequis :",
            stack_size: "Taille de l'ensemble:",
            quality: "Qualité:",
            sockets: "Châsses:",
            gem_level: "Niveau:",
            grants_skill: "Octroie l'Aptitude:",
            rarities: [
                "Normal",
                "Magique",
                "Rare",
                "Unique",
                "Objet monétaire",
                "Gemme",
                "Carte divinatoire",
                "Quête",
            ],
            modifiers: &[
                ("Modificateur d'Objet unique", "Unique Modifier"),
                ("Modificateur vaal unique", "Unique Modifier"),
                ("Modificateur Implicite", "Implicit Modifier"),
                ("Amélioration de Corruption", "Corrupted Implicit Modifier"),
                ("Préfixe", "Prefix Modifier"),
                ("Suffixe", "Suffix Modifier"),
                ("Amélioration", "Enchant Modifier"),
                ("Artisanat", "Crafted Modifier"),
                ("Fissuré", "Fractured Modifier"),
                ("Profané", "Desecrated Modifier"),
            ],
            flags: &[
                ("Double Corruption", "Double Corrupted"),
                ("Corrompu", "Corrupted"),
                ("Reflété", "Mirrored"),
                ("Sanctifié", "Sanctified"),
                ("Non modifiable", "Unmodifiable"),
            ],
            cannot_use: "Vous ne pouvez pas utiliser cet objet",
            unscalable: " — Valeur fixe",
            negations: &[
                ("réduction", "augmentation"),
                ("réduit", "augmenté"),
                ("moins", "plus"),
            ],
            class_stems: &[
                ("non taillé", "Uncut Skill Gem"),
                ("essence", "Essence"),
                ("monétaire", "Stackable Currency"),
            ],
        },
    ),
    (
        Lang::Es,
        ClientStrings {
            item_class: "Clase de objeto:",
            rarity: "Rareza:",
            item_level: "Nivel de objeto:",
            requires: "Requiere:",
            stack_size: "Tamaño de la pila:",
            quality: "Calidad:",
            sockets: "Engarces:",
            gem_level: "Nivel:",
            grants_skill: "Otorga la habilidad:",
            rarities: [
                "Normal",
                "Mágico",
                "Raro",
                "Único",
                "Objetos monetarios",
                "Gema",
                "Carta de adivinación",
                "Misión",
            ],
            modifiers: &[
                ("Modificador único vaal", "Unique Modifier"),
                ("Mejoría de corrupción", "Corrupted Implicit Modifier"),
                ("Mod. de prefijo", "Prefix Modifier"),
                ("Mod. de sufijo", "Suffix Modifier"),
                ("Mod. implícito", "Implicit Modifier"),
                ("de fabricación", "Crafted Modifier"),
                ("Mod. único", "Unique Modifier"),
                ("fracturado", "Fractured Modifier"),
                ("profanado", "Desecrated Modifier"),
                ("Mejoría", "Enchant Modifier"),
            ],
            flags: &[
                ("Doblemente corrupto", "Double Corrupted"),
                ("Corrupto", "Corrupted"),
                ("Reflejado", "Mirrored"),
                ("Santificado", "Sanctified"),
                ("No se puede modificar", "Unmodifiable"),
            ],
            cannot_use: "No puedes usar este objeto",
            unscalable: " — Valor fijo",
            negations: &[
                ("reducción", "aumento"),
                ("reducido", "aumentado"),
                ("menos", "más"),
            ],
            class_stems: &[
                ("sin tallar", "Uncut Skill Gem"),
                ("esencia", "Essence"),
                ("monetari", "Stackable Currency"),
            ],
        },
    ),
    (
        Lang::Pt,
        ClientStrings {
            item_class: "Classe do Item:",
            rarity: "Raridade:",
            item_level: "Nível do Item:",
            requires: "Requer:",
            stack_size: "Tamanho da Pilha:",
            quality: "Qualidade:",
            sockets: "Encaixes:",
            gem_level: "Nível:",
            grants_skill: "Concede Habilidade:",
            rarities: [
                "Normal",
                "Mágico",
                "Raro",
                "Único",
                "Moeda",
                "Gema",
                "Carta de Adivinhação",
                "Missão",
            ],
            modifiers: &[
                ("Modificador Único Vaal", "Unique Modifier"),
                ("Modificador de Prefixo", "Prefix Modifier"),
                ("Modificador de Sufixo", "Suffix Modifier"),
                ("Modificador Implícito", "Implicit Modifier"),
                ("Melhoria de Corrupção", "Corrupted Implicit Modifier"),
                ("Modificador Único", "Unique Modifier"),
                ("Profanado", "Desecrated Modifier"),
                ("Melhoria", "Enchant Modifier"),
                ("Rompido", "Fractured Modifier"),
                ("Criou", "Crafted Modifier"),
            ],
            flags: &[
                ("Corrompido Duas Vezes", "Double Corrupted"),
                ("Corrompido", "Corrupted"),
                ("Espelhado", "Mirrored"),
                ("Santificado", "Sanctified"),
                ("Não Modificável", "Unmodifiable"),
            ],
            cannot_use: "Você não pode usar este item",
            unscalable: " — Valor não escalável",
            negations: &[
                ("reduzido", "aumentado"),
                ("redução", "aumento"),
                ("menos", "mais"),
            ],
            class_stems: &[
                ("não lapidad", "Uncut Skill Gem"),
                ("essência", "Essence"),
                ("moeda", "Stackable Currency"),
                ("anéis", "Rings"),
            ],
        },
    ),
    (
        Lang::Ja,
        ClientStrings {
            item_class: "アイテムクラス:",
            rarity: "レアリティ:",
            item_level: "アイテムレベル:",
            requires: "装備条件：",
            stack_size: "スタック数:",
            quality: "品質:",
            sockets: "ソケット:",
            gem_level: "レベル:",
            grants_skill: "スキルを付与:",
            rarities: [
                "ノーマル",
                "マジック",
                "レア",
                "ユニーク",
                "カレンシー",
                "ジェム",
                "占いカード",
                "クエスト",
            ],
            modifiers: &[
                ("ヴァールユニークモッド", "Unique Modifier"),
                ("コラプトエンハンス", "Corrupted Implicit Modifier"),
                ("プレフィックスモッド", "Prefix Modifier"),
                ("サフィックスモッド", "Suffix Modifier"),
                ("ユニークモッド", "Unique Modifier"),
                ("フラクチャー", "Fractured Modifier"),
                ("エンハンス", "Enchant Modifier"),
                ("暗黙モッド", "Implicit Modifier"),
                ("クラフト", "Crafted Modifier"),
                ("冒涜", "Desecrated Modifier"),
            ],
            flags: &[
                ("ダブルコラプト状態", "Double Corrupted"),
                ("コラプト状態", "Corrupted"),
                ("ミラー状態", "Mirrored"),
                ("聖別化", "Sanctified"),
                ("変更不可", "Unmodifiable"),
            ],
            cannot_use: "このアイテムを使用できません",
            unscalable: " — スケールできない値",
            negations: &[("減少", "増加")],
            class_stems: &[
                ("エッセンス", "Essence"),
                ("カレンシー", "Stackable Currency"),
            ],
        },
    ),
    (
        Lang::Ko,
        ClientStrings {
            item_class: "아이템 종류:",
            rarity: "아이템 희귀도:",
            item_level: "아이템 레벨:",
            requires: "요구 사항:",
            stack_size: "중첩 개수:",
            quality: "퀄리티:",
            sockets: "홈:",
            gem_level: "레벨:",
            grants_skill: "스킬 부여:",
            rarities: [
                "일반",
                "마법",
                "희귀",
                "고유",
                "화폐",
                "젬",
                "점술 카드",
                "퀘스트",
            ],
            modifiers: &[
                ("접두어 속성 부여", "Prefix Modifier"),
                ("접미어 속성 부여", "Suffix Modifier"),
                ("고유 속성 부여", "Unique Modifier"),
                ("고정 속성 부여", "Implicit Modifier"),
                ("바알 고유 속성", "Unique Modifier"),
                ("타락 향상", "Corrupted Implicit Modifier"),
                ("제작된", "Crafted Modifier"),
                ("분열된", "Fractured Modifier"),
                ("훼손된", "Desecrated Modifier"),
                ("향상", "Enchant Modifier"),
            ],
            flags: &[
                ("두 번 타락", "Double Corrupted"),
                ("속성 부여 불가", "Unmodifiable"),
                ("타락", "Corrupted"),
                ("복제", "Mirrored"),
                ("축성", "Sanctified"),
            ],
            cannot_use: "아이템 착용 불가",
            unscalable: " — 변경이 불가능한 값",
            negations: &[("감소", "증가")],
            class_stems: &[("에센스", "Essence"), ("화폐", "Stackable Currency")],
        },
    ),
];

/// trade2 category id → the English in-game item class the rest of the pipeline
/// expects (`gear::category_for`, `parse::BULK_CLASSES`, `danger::is_waystone`).
const CATEGORY_CLASSES: &[(&str, &str)] = &[
    ("armour.gloves", "Gloves"),
    ("armour.chest", "Body Armours"),
    ("armour.helmet", "Helmets"),
    ("armour.boots", "Boots"),
    ("accessory.amulet", "Amulets"),
    ("accessory.ring", "Rings"),
    ("accessory.belt", "Belts"),
    ("armour.quiver", "Quivers"),
    ("armour.shield", "Shields"),
    ("armour.focus", "Foci"),
    ("armour.buckler", "Bucklers"),
    ("weapon.bow", "Bows"),
    ("weapon.crossbow", "Crossbows"),
    ("weapon.wand", "Wands"),
    ("weapon.sceptre", "Sceptres"),
    ("weapon.staff", "Staves"),
    ("weapon.twomace", "Two Hand Maces"),
    ("weapon.twosword", "Two Hand Swords"),
    ("weapon.twoaxe", "Two Hand Axes"),
    ("weapon.onemace", "One Hand Maces"),
    ("weapon.onesword", "One Hand Swords"),
    ("weapon.oneaxe", "One Hand Axes"),
    ("weapon.claw", "Claws"),
    ("weapon.dagger", "Daggers"),
    ("weapon.spear", "Spears"),
    ("weapon.flail", "Flails"),
    ("weapon.warstaff", "Quarterstaves"),
    ("weapon.talisman", "Talismans"),
    ("flask.life", "Life Flasks"),
    ("flask.mana", "Mana Flasks"),
    ("flask.charm", "Charms"),
    ("jewel", "Jewels"),
    ("map.waystone", "Waystones"),
    ("map.tablet", "Tablet"),
    ("map.fragment", "Map Fragments"),
    ("map.breachstone", "Breachstones"),
    ("map.bosskey", "Pinnacle Keys"),
    ("gem.activegem", "Skill Gems"),
    ("gem.supportgem", "Support Gems"),
    ("gem.metagem", "Meta Gems"),
    ("currency.omen", "Omen"),
    ("currency.rune", "Rune"),
    ("currency.soulcore", "Soul Core"),
    ("currency.idol", "Idol"),
    ("currency.socketable", "Augment"),
    ("currency", "Stackable Currency"),
];

/// A Russian stat template, keyed by its normalized text.
struct StatTemplate {
    /// Per slot in the Russian template: `true` = `#` placeholder, `false` = literal.
    slots: Vec<bool>,
    /// English template text (`#` placeholders, `(Local)` tail stripped).
    en: String,
}

/// Russian → English lookup tables built from the trade2 reference data.
#[derive(Default)]
pub struct LangDict {
    /// The copy language these tables translate from.
    lang: Lang,
    stats: HashMap<String, StatTemplate>,
    /// Exact-name lookups: static (currency/bulk), unique names, base types.
    names: HashMap<String, String>,
    /// The reverse of `names`: normalized English → Russian, for English-sourced
    /// lists (the poe.ninja price sheet).
    en_names: HashMap<String, String>,
    /// Base types `(ru, en)`, longest Russian first, for magic/normal name lines.
    bases: Vec<(String, String)>,
    /// Category filter labels: (normalized words, category id).
    categories: Vec<(Vec<String>, String)>,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

/// The language of a localized item copy (by its item-class header), or `None` for an
/// English copy / non-item text.
pub fn detect(text: &str) -> Option<Lang> {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    CLIENT_STRINGS
        .iter()
        .find(|(_, s)| first.starts_with(s.item_class))
        .map(|(l, _)| *l)
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace('ё', "е")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// English name key: poe.ninja-derived names lose apostrophes (`Craiceanns`), so
/// drop them on both sides.
fn norm_en(s: &str) -> String {
    norm(&s.replace(['\'', '’'], ""))
}

/// Normalized lookup key for a stat line/template: numbers and `#` collapse to `#`,
/// signs dropped, trailing `(…)` note removed.
fn stat_key(text: &str) -> String {
    let t = TRAILING_PAREN_RE.replace(text.trim(), "");
    norm(&SLOT_RE.replace_all(&t, "#"))
}

impl LangDict {
    /// Build from the parsed trade2 JSON bodies (`result` arrays included): English
    /// (`www.`) and `lang`'s own host.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        lang: Lang,
        stats_en: &Value,
        stats_ru: &Value,
        items_en: &Value,
        items_ru: &Value,
        static_en: &Value,
        static_ru: &Value,
        filters_ru: &Value,
    ) -> Self {
        let mut d = LangDict {
            lang,
            ..LangDict::default()
        };
        d.build_stats(stats_en, stats_ru);
        d.build_static(static_en, static_ru);
        d.build_items(items_en, items_ru);
        d.build_categories(filters_ru);
        d
    }

    fn build_stats(&mut self, en: &Value, ru: &Value) {
        let mut en_by_id: HashMap<&str, &str> = HashMap::new();
        for e in entries(en, "entries") {
            if let (Some(id), Some(t)) = (e["id"].as_str(), e["text"].as_str()) {
                en_by_id.insert(id, t);
            }
        }
        for group in result(ru) {
            // Pseudo totals never appear on a copy; skip so they can't shadow real ids.
            if group["id"].as_str() == Some("pseudo") {
                continue;
            }
            for e in group["entries"].as_array().into_iter().flatten() {
                let (Some(id), Some(ru_text)) = (e["id"].as_str(), e["text"].as_str()) else {
                    continue;
                };
                let Some(en_text) = en_by_id.get(id) else {
                    continue;
                };
                // Multi-line stats (57 of them) span several copied lines; not handled.
                if ru_text.contains('\n') || en_text.contains('\n') {
                    continue;
                }
                let ru_clean = TRAILING_PAREN_RE.replace(ru_text, "");
                let en_clean = TRAILING_PAREN_RE.replace(en_text, "").trim().to_string();
                let slots: Vec<bool> = SLOT_RE
                    .find_iter(&ru_clean)
                    .map(|m| m.as_str().ends_with('#'))
                    .collect();
                let placeholders = slots.iter().filter(|p| **p).count();
                if en_clean.matches('#').count() != placeholders {
                    continue;
                }
                // First id wins: explicit precedes implicit/rune/… in the table.
                self.stats.entry(stat_key(ru_text)).or_insert(StatTemplate {
                    slots,
                    en: en_clean,
                });
            }
        }
    }

    fn build_static(&mut self, en: &Value, ru: &Value) {
        let mut ru_by_id: HashMap<&str, &str> = HashMap::new();
        for e in entries(ru, "entries") {
            if let (Some(id), Some(t)) = (e["id"].as_str(), e["text"].as_str()) {
                ru_by_id.insert(id, t);
            }
        }
        for e in entries(en, "entries") {
            if let (Some(id), Some(t)) = (e["id"].as_str(), e["text"].as_str()) {
                if let Some(r) = ru_by_id.get(id) {
                    self.add_name(r, t);
                }
            }
        }
    }

    fn build_items(&mut self, en: &Value, ru: &Value) {
        let ru_cats: HashMap<&str, &Value> = result(ru)
            .iter()
            .filter_map(|c| Some((c["id"].as_str()?, c)))
            .collect();
        let mut bases: HashMap<String, String> = HashMap::new();
        for ce in result(en) {
            let Some(cr) = ce["id"].as_str().and_then(|id| ru_cats.get(id)) else {
                continue;
            };
            let list = |c: &Value, uniques: bool| -> Vec<Value> {
                c["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e.get("name").is_some() == uniques)
                    .cloned()
                    .collect()
            };
            let (ue, ur) = (list(ce, true), list(cr, true));
            let mut anchors: HashMap<String, String> = HashMap::new();
            if ue.len() == ur.len() {
                for (a, b) in ue.iter().zip(&ur) {
                    let (Some(an), Some(bn), Some(at), Some(bt)) = (
                        a["name"].as_str(),
                        b["name"].as_str(),
                        a["type"].as_str(),
                        b["type"].as_str(),
                    ) else {
                        continue;
                    };
                    self.add_name(bn, an);
                    anchors
                        .entry(bt.to_string())
                        .or_insert_with(|| at.to_string());
                }
            }
            let types = |v: Vec<Value>| -> Vec<String> {
                v.iter()
                    .filter_map(|e| e["type"].as_str().map(String::from))
                    .collect()
            };
            let (be, br) = (types(list(ce, false)), types(list(cr, false)));
            for (r, e) in align_bases(&be, &br, &anchors) {
                bases.entry(r).or_insert(e);
            }
        }
        for (r, e) in &bases {
            self.add_name(r, e);
        }
        let mut list: Vec<(String, String)> = bases
            .into_iter()
            .filter(|(r, _)| r.chars().count() > 3)
            .map(|(r, e)| (norm(&r), e))
            .collect();
        list.sort_by_key(|(r, _)| std::cmp::Reverse(r.chars().count()));
        self.bases = list;
    }

    fn add_name(&mut self, ru: &str, en: &str) {
        self.names.entry(norm(ru)).or_insert_with(|| en.to_string());
        self.en_names
            .entry(norm_en(en))
            .or_insert_with(|| ru.to_string());
    }

    /// Russian name for an English item/currency name; `"Name (note)"` translates the
    /// name and keeps a note it can't translate.
    pub fn local_name(&self, en: &str) -> Option<String> {
        if let Some(ru) = self.en_names.get(&norm_en(en)) {
            return Some(ru.clone());
        }
        let (head, tail) = en.rsplit_once(" (")?;
        let note = tail.strip_suffix(')')?;
        let head_ru = self.en_names.get(&norm_en(head))?;
        let note_ru = self
            .en_names
            .get(&norm_en(note))
            .map_or(note, String::as_str);
        Some(format!("{head_ru} ({note_ru})"))
    }

    fn build_categories(&mut self, filters_ru: &Value) {
        for group in result(filters_ru) {
            for f in group["filters"].as_array().into_iter().flatten() {
                if f["id"].as_str() != Some("category") {
                    continue;
                }
                for o in f["option"]["options"].as_array().into_iter().flatten() {
                    if let (Some(id), Some(text)) = (o["id"].as_str(), o["text"].as_str()) {
                        let words = norm(text).split(' ').map(String::from).collect();
                        self.categories.push((words, id.to_string()));
                    }
                }
            }
        }
    }

    fn s(&self) -> &'static ClientStrings {
        self.lang.strings().unwrap_or(&CLIENT_STRINGS[0].1)
    }

    /// Localized copied item class → English class name; `None` = no match (the
    /// caller then falls back to the class of the translated base type).
    fn class(&self, local: &str) -> Option<String> {
        let n = norm(local);
        for (stem, en) in self.s().class_stems {
            if n.contains(stem) {
                if *en == "Uncut Skill Gem" && SUPPORT_WORDS.iter().any(|w| n.contains(w)) {
                    return Some("Uncut Support Gem".into());
                }
                return Some(en.to_string());
            }
        }
        let words: Vec<&str> = n.split(' ').collect();
        let mut best: Option<((usize, isize), &str)> = None;
        for (cat_words, id) in &self.categories {
            // Only concrete categories with a known class; the generic `Любой доспех`
            // (armour) would otherwise tie with `Нательная броня` on `доспехи`.
            if !CATEGORY_CLASSES.iter().any(|(c, _)| c == id) {
                continue;
            }
            let matched = words
                .iter()
                .filter(|w| cat_words.iter().any(|c| stem_eq(w, c)))
                .count();
            if matched == 0 {
                continue;
            }
            let unmatched = cat_words
                .iter()
                .filter(|c| !words.iter().any(|w| stem_eq(w, c)))
                .count();
            let score = (matched, -(unmatched as isize));
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, id));
            }
        }
        let (_, id) = best?;
        CATEGORY_CLASSES
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, en)| en.to_string())
    }

    fn name(&self, ru: &str) -> Option<String> {
        self.names.get(&norm(ru)).cloned()
    }

    /// Longest known base type contained in a magic/normal name line.
    fn base_in(&self, line: &str) -> Option<String> {
        let n = norm(line);
        self.bases
            .iter()
            .find(|(r, _)| n.contains(r.as_str()))
            .map(|(_, e)| e.clone())
    }

    /// Translate one modifier line (ranges already stripped). `None` = no template.
    fn stat_line(&self, line: &str) -> Option<String> {
        if let Some(en) = self.fill(line) {
            return Some(en);
        }
        let lower = line.to_lowercase();
        for (neg, pos) in self.s().negations {
            if lower.contains(neg) {
                if let Some(en) = self.fill(&lower.replacen(neg, pos, 1)) {
                    return Some(
                        en.replacen("increased", "reduced", 1)
                            .replacen("more", "less", 1),
                    );
                }
            }
        }
        None
    }

    fn fill(&self, line: &str) -> Option<String> {
        let t = self.stats.get(&stat_key(line))?;
        let nums: Vec<&str> = NUM_RE.find_iter(line).map(|m| m.as_str()).collect();
        if nums.len() != t.slots.len() {
            return None;
        }
        let mut values = nums
            .iter()
            .zip(&t.slots)
            .filter(|(_, ph)| **ph)
            .map(|(n, _)| n.replace(',', "."));
        let mut out = String::new();
        let mut rest = t.en.as_str();
        while let Some(i) = rest.find('#') {
            let mut head = &rest[..i];
            let v = values.next()?;
            // The template may carry its own sign (`+#%`); don't double it.
            if (head.ends_with('+') || head.ends_with('-'))
                && (v.starts_with('+') || v.starts_with('-'))
            {
                head = &head[..head.len() - 1];
            }
            out.push_str(head);
            out.push_str(&v);
            rest = &rest[i + 1..];
        }
        out.push_str(rest);
        Some(out)
    }

    /// Translate a Russian item copy into the English clipboard format.
    #[cfg(test)]
    pub fn translate(&self, text: &str) -> String {
        self.translate_labeled(text, None).0
    }

    /// [`translate`](Self::translate) plus a map from each translated English line
    /// (class, names, modifier lines — as the parser will see them) back to its
    /// original, so the overlay can show the player's own wording while the search
    /// still runs on the English ids. `base_classes` (English base type → class, from
    /// RePoE) resolves item classes the category labels can't; the returned flag is
    /// `false` when the class stayed unresolved.
    pub fn translate_labeled(
        &self,
        text: &str,
        base_classes: Option<&HashMap<String, String>>,
    ) -> (String, HashMap<String, String>, bool) {
        let s = self.s();
        // The unmet-requirements notice (+ its separator) sits between Rarity and the
        // name block; translate it to the English notice the parser drops.
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .map(|l| {
                if l.starts_with(s.cannot_use) {
                    super::parse::CANNOT_USE
                } else {
                    l
                }
            })
            .collect();
        let mut out: Vec<String> = Vec::with_capacity(lines.len());
        let mut labels: HashMap<String, String> = HashMap::new();

        let header_val = |key: &str| {
            lines
                .iter()
                .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
                .unwrap_or_default()
        };
        let rarity_local = header_val(s.rarity);
        let rarity = s
            .rarities
            .iter()
            .position(|r| r.eq_ignore_ascii_case(&rarity_local))
            .map(|i| EN_RARITIES[i].to_string())
            .unwrap_or_else(|| rarity_local.clone());
        let class_local = header_val(s.item_class);

        // The name block: lines after Rarity (and the notice + its separator, if any)
        // up to the next separator. Translated first — the base type can decide the class.
        let ridx = lines.iter().position(|l| l.starts_with(s.rarity)).map(|r| {
            if lines.get(r + 1) == Some(&super::parse::CANNOT_USE) {
                r + 2
            } else {
                r
            }
        });
        let name_end = ridx.map(|r| {
            lines[r + 1..]
                .iter()
                .position(|l| l.starts_with("--------"))
                .map_or(lines.len(), |p| r + 1 + p)
        });
        let mut names: HashMap<usize, String> = HashMap::new();
        if let (Some(r), Some(end)) = (ridx, name_end) {
            for (i, line) in lines.iter().enumerate().take(end).skip(r + 1) {
                // Rare names are random — kept as-is for display.
                let en = if rarity == "Rare" && i == r + 1 {
                    line.to_string()
                } else {
                    self.name(line)
                        .or_else(|| self.base_in(line))
                        .unwrap_or_else(|| line.to_string())
                };
                names.insert(i, en);
            }
        }
        let en_base = name_end.and_then(|e| names.get(&(e.saturating_sub(1))).cloned());

        let mut class_known = true;
        let class = self
            .class(&class_local)
            .or_else(|| base_classes.and_then(|m| m.get(en_base.as_deref()?).cloned()))
            .or_else(|| (rarity == "Currency").then(|| "Stackable Currency".to_string()))
            .unwrap_or_else(|| {
                class_known = false;
                eprintln!("[i18n] unknown item class {class_local:?} (base {en_base:?})");
                class_local.clone()
            });
        labels.insert(class.clone(), class_local);
        labels.insert(rarity.clone(), rarity_local);

        for (i, line) in lines.iter().enumerate() {
            if line.starts_with(s.item_class) {
                out.push(format!("Item Class: {class}"));
                continue;
            }
            if line.starts_with(s.rarity) {
                out.push(format!("Rarity: {rarity}"));
                continue;
            }
            if let Some(en) = names.get(&i) {
                labels.insert(en.clone(), line.to_string());
                out.push(en.clone());
                continue;
            }
            let (en, local) = self.body_line(line);
            if let Some(local) = local {
                labels.insert(en.clone(), local);
            }
            out.push(en);
        }
        labels.retain(|en, local| en != local && !local.is_empty());
        (out.join("\n"), labels, class_known)
    }

    /// One non-header line → (English line, original display label for modifier lines).
    fn body_line(&self, line: &str) -> (String, Option<String>) {
        let s = self.s();
        if line.is_empty() || line.starts_with("--------") {
            return (line.to_string(), None);
        }
        if let Some((_, en)) = s.flags.iter().find(|(f, _)| line == *f) {
            return (en.to_string(), None);
        }
        if line.starts_with('{') {
            return (self.modifier_header(line), None);
        }
        let headers = [
            (s.item_level, "Item Level:"),
            (s.requires, "Requires:"),
            (s.stack_size, "Stack Size:"),
            (s.quality, "Quality:"),
            (s.sockets, "Sockets:"),
            (s.gem_level, "Level:"),
            (s.grants_skill, "Grants Skill:"),
        ];
        for (local, en) in headers {
            if let Some(rest) = line.strip_prefix(local) {
                return (format!("{en} {}", rest.trim()), None);
            }
        }
        // Modifier line: strip the copy decorations, translate, re-attach the marker.
        let no_ranges = RANGE_RE.replace_all(line, "$1");
        let no_unscalable = no_ranges.trim_end_matches(s.unscalable).trim_end();
        let (core, marker) = match MARKER_RE.find(no_unscalable) {
            Some(m) => (&no_unscalable[..m.start()], m.as_str()),
            None => (no_unscalable, ""),
        };
        match self.stat_line(core) {
            Some(en) => (format!("{en}{marker}"), Some(format!("{core}{marker}"))),
            // Other `key: value` properties stay as they are — the parser skips them
            // (colon lines that ARE stats, like rune `Bonded:` lines, matched above).
            None => (line.to_string(), None),
        }
    }

    /// `{ Префикс "Герцогский" (Уровень: 3) — Броня }` → `{ Prefix Modifier "Герцогский" (Tier: 3) }`.
    fn modifier_header(&self, line: &str) -> String {
        let s = self.s();
        let inner = line.trim_start_matches('{').trim_end_matches('}').trim();
        let lower = inner.to_lowercase();
        let Some((local, en)) = s
            .modifiers
            .iter()
            .find(|(m, _)| lower.starts_with(&m.to_lowercase()))
        else {
            return line.to_string();
        };
        let rest = &inner[local.len()..];
        let rest = rest.split(" — ").next().unwrap_or(rest).trim_start();
        // Crafted / desecrated headers repeat the affix side: `Очернённое Суффикс "…"`.
        let side = s
            .modifiers
            .iter()
            .filter(|(_, e)| *e == "Prefix Modifier" || *e == "Suffix Modifier")
            .find(|(m, _)| rest.to_lowercase().starts_with(&m.to_lowercase()));
        let rest = side.map_or(rest, |(m, _)| rest[m.len()..].trim_start());
        let rest = TIER_RE.replace(rest, "(Tier: $1)");
        let rest = rest.trim();
        if rest.is_empty() {
            format!("{{ {en} }}")
        } else {
            format!("{{ {en} {rest} }}")
        }
    }
}

/// "Support" in each client language — tells uncut support gems from uncut skill gems.
const SUPPORT_WORDS: &[&str] = &[
    "поддерж",
    "unterstütz",
    "soutien",
    "apoyo",
    "suporte",
    "サポート",
    "보조",
    "support",
];

/// Two words name the same thing if equal, or if they share a stem: a common prefix of
/// at least 3 chars covering ≥60% of the shorter word (`кольца`/`кольцо`, `Ringe`/`Ring`).
fn stem_eq(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let common = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let shorter = a.chars().count().min(b.chars().count());
    common >= 3 && common * 10 >= shorter * 6
}

/// Pair same-category base lists. Equal length → by position; otherwise only the
/// stretches between two unique-derived anchors with the same index offset are
/// trusted, plus the anchors themselves.
fn align_bases(
    en: &[String],
    ru: &[String],
    anchors: &HashMap<String, String>,
) -> Vec<(String, String)> {
    if en.len() == ru.len() {
        return ru.iter().cloned().zip(en.iter().cloned()).collect();
    }
    let en_idx: HashMap<&str, usize> = en
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut points: Vec<(usize, usize)> = ru
        .iter()
        .enumerate()
        .filter_map(|(i, r)| Some((i, *en_idx.get(anchors.get(r)?.as_str())?)))
        .collect();
    points.sort_unstable();
    let mut pairs: HashMap<String, String> = HashMap::new();
    for w in points.windows(2) {
        let ((i1, j1), (i2, j2)) = (w[0], w[1]);
        if i1 as isize - j1 as isize == i2 as isize - j2 as isize {
            for k in i1..=i2 {
                let j = (k as isize - (i1 as isize - j1 as isize)) as usize;
                if let Some(e) = en.get(j) {
                    pairs.insert(ru[k].clone(), e.clone());
                }
            }
        }
    }
    for (r, e) in anchors {
        pairs.insert(r.clone(), e.clone());
    }
    pairs.into_iter().collect()
}

fn result(v: &Value) -> &[Value] {
    v["result"].as_array().map(Vec::as_slice).unwrap_or(&[])
}

/// Flatten `result[*].<key>[*]`.
fn entries<'a>(v: &'a Value, key: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    result(v)
        .iter()
        .flat_map(move |c| c[key].as_array().into_iter().flatten())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dict() -> LangDict {
        let stats_en = json!({"result": [{"id": "explicit", "entries": [
            {"id": "explicit.stat_3299347043", "text": "# to maximum Life"},
            {"id": "explicit.stat_3372524247", "text": "#% to Fire Resistance"},
            {"id": "explicit.stat_1940865751", "text": "Adds # to # Cold Damage (Local)"},
            {"id": "explicit.stat_3639275092", "text": "#% increased Attribute Requirements"},
        ]}]});
        let stats_ru = json!({"result": [{"id": "explicit", "entries": [
            {"id": "explicit.stat_3299347043", "text": "# к максимуму здоровья"},
            {"id": "explicit.stat_3372524247", "text": "#% к сопротивлению огню"},
            {"id": "explicit.stat_1940865751", "text": "Добавляет от # до # урона от холода (На предмете)"},
            {"id": "explicit.stat_3639275092", "text": "#% повышение требований к характеристикам"},
        ]}]});
        let items_en = json!({"result": [{"id": "accessory", "entries": [
            {"name": "Andvarius", "type": "Gold Ring"}, {"type": "Gold Ring"}, {"type": "Sapphire Ring"}]}]});
        let items_ru = json!({"result": [{"id": "accessory", "entries": [
            {"name": "Андвариус", "type": "Золотое кольцо"}, {"type": "Золотое кольцо"}, {"type": "Кольцо с сапфиром"}]}]});
        let static_en = json!({"result": [{"id": "Currency", "entries": [{"id": "chaos", "text": "Chaos Orb"}]}]});
        let static_ru = json!({"result": [{"id": "Currency", "entries": [{"id": "chaos", "text": "Сфера хаоса"}]}]});
        let filters_ru = json!({"result": [{"filters": [{"id": "category", "option": {"options": [
            {"id": "armour", "text": "Любой доспех"},
            {"id": "jewel", "text": "Любой самоцвет"},
            {"id": "accessory.ring", "text": "Кольцо"},
            {"id": "armour.chest", "text": "Нательная броня"},
            {"id": "weapon.twomace", "text": "Двуручная булава"},
            {"id": "weapon.onemace", "text": "Одноручная булава"},
            {"id": "weapon.staff", "text": "Посох"},
            {"id": "weapon.warstaff", "text": "Боевой посох"},
            {"id": "map.waystone", "text": "Путевой камень"},
        ]}}]}]});
        LangDict::build(
            Lang::Ru,
            &stats_en,
            &stats_ru,
            &items_en,
            &items_ru,
            &static_en,
            &static_ru,
            &filters_ru,
        )
    }

    #[test]
    fn classes_by_stem() {
        let d = dict();
        assert_eq!(d.class("Кольца").as_deref(), Some("Rings"));
        assert_eq!(
            d.class("Нательные доспехи").as_deref(),
            Some("Body Armours")
        );
        assert_eq!(
            d.class("Двуручные булавы").as_deref(),
            Some("Two Hand Maces")
        );
        assert_eq!(d.class("Посохи").as_deref(), Some("Staves"));
        assert_eq!(d.class("Боевые посохи").as_deref(), Some("Quarterstaves"));
        assert_eq!(d.class("Путевые камни").as_deref(), Some("Waystones"));
        assert_eq!(d.class("Самоцветы").as_deref(), Some("Jewels"));
        assert_eq!(
            d.class("Стопки валюты").as_deref(),
            Some("Stackable Currency")
        );
    }

    #[test]
    fn rare_ring_translates() {
        let d = dict();
        let ru =
            "Класс предмета: Кольца\nРедкость: Редкий\nПетля бури\nКольцо с сапфиром\n--------\n\
                  Требуется: Уровень 40\n--------\nУровень предмета: 79\n--------\n\
                  +25% к сопротивлению огню (implicit)\n--------\n+60(55-64) к максимуму здоровья\n\
                  35% снижение требований к характеристикам";
        let en = d.translate(ru);
        assert_eq!(
            en,
            "Item Class: Rings\nRarity: Rare\nПетля бури\nSapphire Ring\n--------\n\
             Requires: Уровень 40\n--------\nItem Level: 79\n--------\n\
             +25% to Fire Resistance (implicit)\n--------\n+60 to maximum Life\n\
             35% reduced Attribute Requirements"
        );
    }

    #[test]
    fn unique_currency_and_magic() {
        let d = dict();
        let en = d.translate("Класс предмета: Кольца\nРедкость: Уникальный\nАндвариус\nЗолотое кольцо\n--------\nОсквернено");
        assert_eq!(
            en,
            "Item Class: Rings\nRarity: Unique\nAndvarius\nGold Ring\n--------\nCorrupted"
        );
        let en = d.translate("Класс предмета: Стопки валюты\nРедкость: Валюта\nСфера хаоса\n--------\nРазмер стопки: 3/20");
        assert_eq!(en, "Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n--------\nStack Size: 3/20");
        let en = d.translate(
            "Класс предмета: Двуручные булавы\nРедкость: Волшебный\nХолодное Кольцо с сапфиром вечности\n--------\n\
             { Префикс \"Холодный\" (Уровень: 7) — Урон, Холод }\nДобавляет от 20(16-23) до 26(25-35) урона от холода",
        );
        assert_eq!(
            en,
            "Item Class: Two Hand Maces\nRarity: Magic\nSapphire Ring\n--------\n\
             { Prefix Modifier \"Холодный\" (Tier: 7) }\nAdds 20 to 26 Cold Damage"
        );
    }

    #[test]
    fn english_names_back_to_russian() {
        let d = dict();
        assert_eq!(d.local_name("Chaos Orb").as_deref(), Some("Сфера хаоса"));
        assert_eq!(
            d.local_name("Andvarius (Gold Ring)").as_deref(),
            Some("Андвариус (Золотое кольцо)")
        );
        assert_eq!(
            d.local_name("Andvarius (relic)").as_deref(),
            Some("Андвариус (relic)")
        );
        assert!(d.local_name("Nonexistent Thing").is_none());
    }

    #[test]
    fn cannot_use_notice_skipped() {
        let d = dict();
        let en = d.translate(
            "Класс предмета: Кольца\nРедкость: Уникальный\n\
             Вы не можете использовать этот предмет, его параметры не будут учтены\n--------\n\
             Андвариус\nЗолотое кольцо\n--------",
        );
        let item = crate::trade::parse_item(&en).expect("parses");
        assert_eq!(item.name, "Andvarius");
        assert_eq!(item.base_type, "Gold Ring");
    }

    #[test]
    fn german_copy_translates_and_is_detected() {
        let stats_en = json!({"result": [{"id": "explicit", "entries": [
            {"id": "explicit.stat_3299347043", "text": "# to maximum Life"},
            {"id": "explicit.stat_3639275092", "text": "#% increased Attribute Requirements"},
        ]}]});
        let stats_de = json!({"result": [{"id": "explicit", "entries": [
            {"id": "explicit.stat_3299347043", "text": "# zu maximalem Leben"},
            {"id": "explicit.stat_3639275092", "text": "#% erhöhte Attributanforderungen"},
        ]}]});
        let items_en =
            json!({"result": [{"id": "accessory", "entries": [{"type": "Sapphire Ring"}]}]});
        let items_de =
            json!({"result": [{"id": "accessory", "entries": [{"type": "Saphirring"}]}]});
        let empty = json!({"result": []});
        let filters_de = json!({"result": [{"filters": [{"id": "category", "option": {"options": [
            {"id": "accessory.ring", "text": "Ring"},
        ]}}]}]});
        let d = LangDict::build(
            Lang::De,
            &stats_en,
            &stats_de,
            &items_en,
            &items_de,
            &empty,
            &empty,
            &filters_de,
        );
        let copy = "Gegenstandsklasse: Ringe\nSeltenheit: Selten\nSturmschlinge\nSaphirring\n--------\n\
                    Gegenstandsstufe: 79\n--------\n{ Suffix-Modifikator \"des Fuchses\" (Stufe: 3) }\n\
                    +60(55-64) zu maximalem Leben\n35% verringerte Attributanforderungen\n--------\nVerderbt";
        assert_eq!(detect(copy), Some(Lang::De));
        assert_eq!(detect("Item Class: Rings\nRarity: Rare"), None);
        assert_eq!(
            d.translate(copy),
            "Item Class: Rings\nRarity: Rare\nSturmschlinge\nSapphire Ring\n--------\n\
             Item Level: 79\n--------\n{ Suffix Modifier \"des Fuchses\" (Tier: 3) }\n\
             +60 to maximum Life\n35% reduced Attribute Requirements\n--------\nCorrupted"
        );
    }

    #[test]
    fn literal_numbers_in_template_are_not_values() {
        let stats_en = json!({"result": [{"id": "rune", "entries": [
            {"id": "rune.stat_1", "text": "# to maximum Life per 8 Armour on Equipped Helmet"}]}]});
        let stats_ru = json!({"result": [{"id": "rune", "entries": [
            {"id": "rune.stat_1", "text": "# к максимуму здоровья за каждые 8 брони на надетом шлеме"}]}]});
        let mut d = LangDict {
            lang: Lang::Ru,
            ..LangDict::default()
        };
        d.build_stats(&stats_en, &stats_ru);
        assert_eq!(
            d.stat_line("+2 к максимуму здоровья за каждые 8 брони на надетом шлеме")
                .as_deref(),
            Some("+2 to maximum Life per 8 Armour on Equipped Helmet")
        );
    }
}

/// Live-data check: `POE2_I18N_DATA=<dir with {www,ru}-{stats,items,static,filters}.json>
/// POE2_I18N_ITEMS=<dir of Russian copies> cargo test real_copies -- --ignored --nocapture`.
#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore]
    fn real_copies() {
        let data = std::env::var("POE2_I18N_DATA").expect("POE2_I18N_DATA");
        let load = |n: &str| -> Value {
            serde_json::from_str(&std::fs::read_to_string(format!("{data}/{n}.json")).unwrap())
                .unwrap()
        };
        let d = LangDict::build(
            Lang::Ru,
            &load("www-stats"),
            &load("ru-stats"),
            &load("www-items"),
            &load("ru-items"),
            &load("www-static"),
            &load("ru-static"),
            &load("ru-filters"),
        );
        let dir = std::env::var("POE2_I18N_ITEMS").expect("POE2_I18N_ITEMS");
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for p in paths {
            let ru = std::fs::read_to_string(&p).unwrap();
            let en = d.translate(&ru);
            println!("===== {}\n{en}", p.display());
            let parsed = crate::trade::parse_item(&en).expect("parses");
            println!(
                "-> class={:?} rarity={:?} name={:?} base={:?} stats={}",
                parsed.item_class,
                parsed.rarity,
                parsed.name,
                parsed.base_type,
                parsed.stats.len()
            );
        }
    }
}

/// Live coverage of English→Russian names: `POE2_I18N_DATA=<dir> POE2_I18N_NAMES=<tsv of
/// category\tname> cargo test ru_name_coverage -- --ignored --nocapture`.
#[cfg(test)]
mod live_names {
    use super::*;

    #[test]
    #[ignore]
    fn ru_name_coverage() {
        let data = std::env::var("POE2_I18N_DATA").expect("POE2_I18N_DATA");
        let load = |n: &str| -> Value {
            serde_json::from_str(&std::fs::read_to_string(format!("{data}/{n}.json")).unwrap())
                .unwrap()
        };
        let d = LangDict::build(
            Lang::Ru,
            &load("www-stats"),
            &load("ru-stats"),
            &load("www-items"),
            &load("ru-items"),
            &load("www-static"),
            &load("ru-static"),
            &load("ru-filters"),
        );
        let names = std::fs::read_to_string(std::env::var("POE2_I18N_NAMES").unwrap()).unwrap();
        let mut per: std::collections::BTreeMap<&str, (usize, usize, Vec<&str>)> =
            Default::default();
        for line in names.lines() {
            let Some((cat, name)) = line.split_once('\t') else {
                continue;
            };
            let e = per.entry(cat).or_default();
            e.1 += 1;
            match d.local_name(name) {
                Some(_) => e.0 += 1,
                None => e.2.push(name),
            }
        }
        for (cat, (hit, all, miss)) in per {
            println!("{cat}: {hit}/{all}  miss: {:?}", &miss[..miss.len().min(5)]);
        }
    }
}
