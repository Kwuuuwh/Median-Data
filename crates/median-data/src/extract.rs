use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde_json::Value;
use sources::de;

use crate::normalize;

/// Raw DE item facts, before consensus.
pub struct DeItem {
    pub unique_name: String,
    pub name: String,
    pub category: String,
    /// The manifest array the object came from, e.g. `ExportAvionics`.
    pub array: String,
    /// DE's own prototype pointer: the type this item inherits from.
    pub parent: Option<String>,
    /// `type` as DE prints it. Only mods carry it.
    pub mod_type: Option<String>,
}

/// A DE foundry recipe.
pub struct DeRecipe {
    pub blueprint: String,
    pub result: String,
    pub ingredients: Vec<(String, i64)>,
    pub build_price: Option<i64>,
    pub build_time: Option<i64>,
    pub consumed: bool,
    pub rush_price: Option<i64>,
}

/// A star-chart node from `ExportRegions`.
pub struct DeRegion {
    /// DE's key for the node, e.g. `SolNode94`.
    pub node: String,
    pub name: String,
    pub location: String,
    pub mission: i64,
    pub faction: i64,
    pub node_type: i64,
    pub mastery: i64,
    pub min_level: i64,
    pub max_level: i64,
}

/// One reward line of a relic.
pub struct DeReward {
    pub relic: String,
    pub reward: String,
    pub rarity: String,
}

/// DE items (uniqueName + name, `productCategory` as category) from a manifest's raw bytes.
pub fn de_items(manifest: &str, raw: &[u8]) -> Result<Vec<DeItem>> {
    let doc = parse(manifest, raw)?;
    let mut out = Vec::new();
    for (array, el) in objects(&doc) {
        let (Some(unique_name), Some(name)) = (
            el.get("uniqueName").and_then(Value::as_str),
            el.get("name").and_then(Value::as_str),
        ) else {
            continue;
        };
        let name = normalize::name(name);
        if unique_name.is_empty() || name.is_empty() {
            continue;
        }
        out.push(DeItem {
            unique_name: unique_name.to_string(),
            name: name.to_string(),
            category: el
                .get("productCategory")
                .and_then(Value::as_str)
                .unwrap_or(manifest)
                .to_string(),
            array: array.to_string(),
            parent: text(el, "parentName"),
            mod_type: text(el, "type"),
        });
    }
    Ok(out)
}

/// Foundry recipes from `ExportRecipes`.
pub fn de_recipes(raw: &[u8]) -> Result<Vec<DeRecipe>> {
    let doc = parse("ExportRecipes", raw)?;
    let mut out = Vec::new();
    for (_, el) in objects(&doc) {
        let (Some(blueprint), Some(result)) = (
            el.get("uniqueName").and_then(Value::as_str),
            el.get("resultType").and_then(Value::as_str),
        ) else {
            continue;
        };
        if blueprint.is_empty() || result.is_empty() {
            continue;
        }
        out.push(DeRecipe {
            blueprint: normalize::path(blueprint).into_owned(),
            result: normalize::path(result).into_owned(),
            ingredients: ingredients(el),
            build_price: int(el, "buildPrice"),
            build_time: int(el, "buildTime"),
            consumed: el
                .get("consumeOnUse")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            rush_price: int(el, "skipBuildTimePrice"),
        });
    }
    Ok(out)
}

/// Relic reward lines from `ExportRelicArcane`.
pub fn de_rewards(raw: &[u8]) -> Result<Vec<DeReward>> {
    let doc = parse("ExportRelicArcane", raw)?;
    let mut out = Vec::new();
    for (_, el) in objects(&doc) {
        let Some(relic) = el.get("uniqueName").and_then(Value::as_str) else {
            continue;
        };
        let Some(rewards) = el.get("relicRewards").and_then(Value::as_array) else {
            continue;
        };
        for r in rewards {
            let Some(reward) = r.get("rewardName").and_then(Value::as_str) else {
                continue;
            };
            out.push(DeReward {
                relic: normalize::path(relic).into_owned(),
                reward: normalize::path(reward).into_owned(),
                rarity: r
                    .get("rarity")
                    .and_then(Value::as_str)
                    .unwrap_or("COMMON")
                    .to_string(),
            });
        }
    }
    Ok(out)
}

/// Star-chart nodes from `ExportRegions`.
pub fn de_regions(raw: &[u8]) -> Result<Vec<DeRegion>> {
    let doc = parse("ExportRegions", raw)?;
    let mut out = Vec::new();
    for (_, el) in objects(&doc) {
        let (Some(node), Some(name)) = (
            el.get("uniqueName").and_then(Value::as_str),
            el.get("name").and_then(Value::as_str),
        ) else {
            continue;
        };
        let name = normalize::name(name);
        if node.is_empty() || name.is_empty() {
            continue;
        }
        out.push(DeRegion {
            node: node.to_string(),
            name: name.to_string(),
            location: el
                .get("systemName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            mission: int(el, "missionIndex").unwrap_or(-1),
            faction: int(el, "factionIndex").unwrap_or(-1),
            node_type: int(el, "nodeType").unwrap_or(-1),
            mastery: int(el, "masteryReq").unwrap_or(0),
            min_level: int(el, "minEnemyLevel").unwrap_or(0),
            max_level: int(el, "maxEnemyLevel").unwrap_or(0),
        });
    }
    Ok(out)
}

/// Icon texture locations from `ExportManifest`, keyed by item.
pub fn de_textures(raw: &[u8]) -> Result<BTreeMap<String, String>> {
    let doc = parse("ExportManifest", raw)?;
    let mut out = BTreeMap::new();
    for (_, el) in objects(&doc) {
        let (Some(unique_name), Some(texture)) = (
            el.get("uniqueName").and_then(Value::as_str),
            el.get("textureLocation").and_then(Value::as_str),
        ) else {
            continue;
        };
        if texture.is_empty() {
            continue;
        }
        out.insert(unique_name.to_string(), texture.replace('\\', "/"));
    }
    Ok(out)
}

fn parse(manifest: &str, raw: &[u8]) -> Result<Value> {
    serde_json::from_str(&de::sanitize(raw)).with_context(|| format!("parse {manifest}"))
}

/// Every object across the manifest's arrays, each with the array it came from.
fn objects(doc: &Value) -> Vec<(&str, &Value)> {
    let Some(obj) = doc.as_object() else {
        return Vec::new();
    };
    obj.iter()
        .filter_map(|(key, value)| value.as_array().map(|a| (key.as_str(), a)))
        .flat_map(|(key, a)| a.iter().map(move |el| (key, el)))
        .collect()
}

/// A non-empty string field.
fn text(el: &Value, key: &str) -> Option<String> {
    el.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn ingredients(el: &Value) -> Vec<(String, i64)> {
    el.get("ingredients")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|ing| {
                    let item = ing.get("ItemType").and_then(Value::as_str)?;
                    if item.is_empty() {
                        return None;
                    }
                    Some((
                        normalize::path(item).into_owned(),
                        int(ing, "ItemCount").unwrap_or(1),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn int(el: &Value, key: &str) -> Option<i64> {
    let v = el.get(key)?;
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_recipe_with_normalized_paths() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "ExportRecipes": [
                { "uniqueName": "/Lotus/StoreItems/Types/Recipes/WarframeRecipes/VoltPrimeBlueprint",
                  "resultType": "/Lotus/Powersuits/Volt/VoltPrime",
                  "buildPrice": 25000, "buildTime": 259200,
                  "consumeOnUse": true, "skipBuildTimePrice": 50,
                  "ingredients": [
                      { "ItemType": "/Lotus/Types/Items/MiscItems/OrokinCell", "ItemCount": 1 },
                      { "ItemType": "", "ItemCount": 3 }
                  ] }
            ]
        }))
        .unwrap();
        let recipes = de_recipes(&raw).unwrap();
        assert_eq!(recipes.len(), 1);
        let r = &recipes[0];
        assert_eq!(
            r.blueprint,
            "/Lotus/Types/Recipes/WarframeRecipes/VoltPrimeBlueprint"
        );
        assert!(r.consumed);
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.ingredients[0].1, 1);
    }

    #[test]
    fn extracts_relic_rewards() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "ExportRelicArcane": [
                { "uniqueName": "/Lotus/Types/Game/Projections/T2VoidProjectionDBronze",
                  "name": "Meso D1 Relic",
                  "relicRewards": [
                      { "rewardName": "/Lotus/StoreItems/Types/Recipes/WarframeRecipes/VoltPrimeChassisBlueprint",
                        "rarity": "UNCOMMON", "tier": 0, "itemCount": 1 }
                  ] },
                { "uniqueName": "/Lotus/Types/Game/Arcane/X", "name": "Arcane Energize" }
            ]
        }))
        .unwrap();
        let rewards = de_rewards(&raw).unwrap();
        assert_eq!(rewards.len(), 1);
        assert_eq!(
            rewards[0].reward,
            "/Lotus/Types/Recipes/WarframeRecipes/VoltPrimeChassisBlueprint"
        );
        assert_eq!(rewards[0].rarity, "UNCOMMON");
    }
}
