use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

use crate::net::{get, get_with};

const ITEMS_URL: &str = "https://api.warframe.market/v2/items";
const ASSETS_URL: &str = "https://warframe.market/static/assets";

/// What the market serves where it has no picture: a question mark on a black square.
const PLACEHOLDER: &str = "items/unknown.png";

/// One warframe.market item.
#[derive(Debug, Clone)]
pub struct WfmItem {
    /// WFM slug / url_name.
    pub slug: String,
    /// DE `uniqueName` this maps to (`gameRef`); the bridge key.
    pub game_ref: Option<String>,
    /// English display name (`i18n.en.name`).
    pub en_name: Option<String>,
    /// Russian display name (`i18n.ru.name`).
    pub ru_name: Option<String>,
    /// Ducat value at the Void trader.
    pub ducats: Option<i64>,
    /// WFM tags (`mod`, `prime`, `set`, ...).
    pub tags: Vec<String>,
    /// Asset path of the item's picture per language. A mod's name and stats are drawn
    /// into it, so the languages carry different images.
    pub icons: BTreeMap<String, String>,
}

impl WfmItem {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// Fetch the full `/v2/items` list as raw bytes, localized to carry Russian too.
pub fn fetch_items(agent: &ureq::Agent) -> Result<Vec<u8>> {
    get_with(agent, ITEMS_URL, &[("Language", "ru")])
}

/// Fetch one static asset by the path an item's `i18n` carries.
pub fn fetch_asset(agent: &ureq::Agent, path: &str) -> Result<Vec<u8>> {
    get(agent, &format!("{ASSETS_URL}/{}", escape(path)))
}

/// Percent-encode an asset path, keeping its separators. Some names carry a typographic
/// apostrophe, which cannot travel in a URL as itself.
fn escape(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Parse a `/v2/items` response body into items.
pub fn parse(raw: &[u8]) -> Result<Vec<WfmItem>> {
    let doc: Value = serde_json::from_slice(raw)?;
    let mut out = Vec::new();
    let Some(data) = doc.get("data").and_then(Value::as_array) else {
        return Ok(out);
    };
    for el in data {
        let Some(slug) = el.get("slug").and_then(Value::as_str) else {
            continue;
        };
        out.push(WfmItem {
            slug: slug.to_string(),
            game_ref: el
                .get("gameRef")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            en_name: i18n_name(el, "en"),
            ru_name: i18n_name(el, "ru"),
            ducats: el.get("ducats").and_then(Value::as_i64),
            tags: el
                .get("tags")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            icons: icons(el),
        });
    }
    Ok(out)
}

fn i18n_name(el: &Value, lang: &str) -> Option<String> {
    el.get("i18n")
        .and_then(|i| i.get(lang))
        .and_then(|l| l.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The picture for each language the listing carries. The thumb is preferred: it is already
/// the size a catalog renders at, and an eighth of the full image.
fn icons(el: &Value) -> BTreeMap<String, String> {
    let Some(langs) = el.get("i18n").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    langs
        .iter()
        .filter_map(|(lang, body)| {
            let path = body
                .get("thumb")
                .or_else(|| body.get("icon"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && *s != PLACEHOLDER)?;
            Some((lang.clone(), path.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bridge_names_ducats_and_tags() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "data": [
                { "slug": "volt_prime_chassis_blueprint",
                  "gameRef": "/Lotus/Types/Recipes/WarframeRecipes/VoltPrimeChassisBlueprint",
                  "tags": ["component", "prime", "blueprint"],
                  "ducats": 45,
                  "i18n": {
                      "en": { "name": "Volt Prime Chassis Blueprint",
                              "icon": "items/images/en/a.png",
                              "thumb": "items/images/en/thumbs/a.128x128.png" },
                      "ru": { "name": "Вольт Прайм: Каркас (Чертеж)",
                              "icon": "items/images/ru/b.png" }
                  } }
            ]
        }))
        .unwrap();
        let items = parse(&raw).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(
            it.game_ref.as_deref(),
            Some("/Lotus/Types/Recipes/WarframeRecipes/VoltPrimeChassisBlueprint")
        );
        assert_eq!(it.ducats, Some(45));
        assert_eq!(it.ru_name.as_deref(), Some("Вольт Прайм: Каркас (Чертеж)"));
        assert!(it.has_tag("prime"));
        assert!(!it.has_tag("set"));
    }

    #[test]
    fn a_thumb_wins_over_the_full_picture_and_falls_back_to_it() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "data": [
                { "slug": "creeping_bullseye",
                  "i18n": {
                      "en": { "icon": "items/images/en/a.png",
                              "thumb": "items/images/en/thumbs/a.128x128.png" },
                      "ru": { "icon": "items/images/ru/b.png" }
                  } }
            ]
        }))
        .unwrap();
        let it = &parse(&raw).unwrap()[0];
        assert_eq!(it.icons["en"], "items/images/en/thumbs/a.128x128.png");
        assert_eq!(it.icons["ru"], "items/images/ru/b.png");
    }

    #[test]
    fn the_markets_placeholder_is_not_a_picture() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "data": [
                { "slug": "untime_rift",
                  "i18n": { "en": { "icon": "items/unknown.png" } } }
            ]
        }))
        .unwrap();
        assert!(parse(&raw).unwrap()[0].icons.is_empty());
    }

    #[test]
    fn an_apostrophe_survives_as_an_escape() {
        assert_eq!(
            escape("items/images/en/summoner’s_wrath.png"),
            "items/images/en/summoner%E2%80%99s_wrath.png"
        );
    }
}
