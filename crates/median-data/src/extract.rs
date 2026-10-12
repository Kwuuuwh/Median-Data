use std::collections::BTreeMap;

use anyhow::{Context, Result};
use graph::{Damage, Weapon, WeaponStats};
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
    /// `masteryReq`: the rank the game asks before the item is built or traded.
    pub mastery_req: Option<i64>,
    /// `maxLevelCap`, which DE prints only where an item ranks past thirty.
    pub max_level_cap: Option<i64>,
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
    /// Copies one build hands over.
    pub output: i64,
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
    /// `masteryReq`: the rank the node asks before it can be played.
    pub mastery_req: i64,
    pub min_level: i64,
    pub max_level: i64,
}

/// One reward line of a relic.
pub struct DeReward {
    pub relic: String,
    pub reward: String,
    pub rarity: String,
    /// How many the slot hands over. Forma comes in twos.
    pub count: i64,
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
            mastery_req: int(el, "masteryReq"),
            max_level_cap: int(el, "maxLevelCap"),
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
            output: int(el, "num").filter(|n| *n > 0).unwrap_or(1),
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
                count: int(r, "itemCount").unwrap_or(1),
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
            mastery_req: int(el, "masteryReq").unwrap_or(0),
            min_level: int(el, "minEnemyLevel").unwrap_or(0),
            max_level: int(el, "maxEnemyLevel").unwrap_or(0),
        });
    }
    Ok(out)
}

/// Weapon facts from `ExportWeapons`, keyed by item. Railjack armaments live in their own
/// array and are left out.
pub fn de_weapons(raw: &[u8]) -> Result<BTreeMap<String, Weapon>> {
    let doc = parse("ExportWeapons", raw)?;
    let mut out = BTreeMap::new();
    let listed = doc.get("ExportWeapons").and_then(Value::as_array);
    for el in listed.into_iter().flatten() {
        let (Some(unique_name), Some(disposition)) = (
            el.get("uniqueName").and_then(Value::as_str),
            real(el, "omegaAttenuation"),
        ) else {
            continue;
        };
        out.insert(
            unique_name.to_string(),
            Weapon {
                disposition,
                disposition_primary: real(el, "primeOmegaAttenuation"),
                stats: weapon_stats(el),
                mounts: Vec::new(),
                riven: None,
            },
        );
    }
    Ok(out)
}

/// A weapon's printed attack. A part prints zero total damage and a damage array of noise.
fn weapon_stats(el: &Value) -> Option<WeaponStats> {
    let total_damage = real(el, "totalDamage").filter(|t| *t > 0.0)?;
    let per_shot = el.get("damagePerShot").and_then(Value::as_array)?;
    let damage = Damage::ALL
        .iter()
        .zip(per_shot)
        .filter_map(|(kind, amount)| Some((*kind, amount.as_f64()?)))
        .filter(|(_, amount)| *amount != 0.0)
        .collect();
    Some(WeaponStats {
        trigger: text(el, "trigger"),
        crit_chance: real(el, "criticalChance")?,
        crit_multiplier: real(el, "criticalMultiplier")?,
        status_chance: real(el, "procChance")?,
        fire_rate: real(el, "fireRate")?,
        multishot: int(el, "multishot"),
        magazine: int(el, "magazineSize"),
        reload_secs: real(el, "reloadTime"),
        total_damage,
        damage,
    })
}

/// The stats one class of riven rolls from, as `ExportUpgrades` lists them.
pub struct DeRivenPool {
    pub unique_name: String,
    pub stats: Vec<DeRivenStat>,
    /// `availableChallenges`: the challenge paths, each with its complication paths.
    pub challenges: Vec<(String, Vec<String>)>,
}

/// One `upgradeEntries` row of a riven.
pub struct DeRivenStat {
    pub tag: String,
    pub prefix: String,
    pub suffix: String,
    pub bases: Vec<f64>,
    /// `locTag` of the first value: the English the client prints.
    pub shown: Option<String>,
    pub reverse_symbol: bool,
}

/// Riven stat pools from `ExportUpgrades`: the records under `/Randomized/` that carry
/// `upgradeEntries`. The veiled rivens carry none.
pub fn de_riven_pools(raw: &[u8]) -> Result<Vec<DeRivenPool>> {
    let doc = parse("ExportUpgrades", raw)?;
    let mut out = Vec::new();
    for (_, el) in objects(&doc) {
        let Some(unique_name) = el
            .get("uniqueName")
            .and_then(Value::as_str)
            .filter(|path| path.contains("/Randomized/"))
        else {
            continue;
        };
        let Some(entries) = el.get("upgradeEntries").and_then(Value::as_array) else {
            continue;
        };
        let stats = entries
            .iter()
            .filter_map(|e| {
                let values: Vec<&Value> = e
                    .get("upgradeValues")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .collect();
                Some(DeRivenStat {
                    tag: text(e, "tag")?,
                    prefix: e.get("prefixTag").and_then(Value::as_str)?.to_string(),
                    suffix: e.get("suffixTag").and_then(Value::as_str)?.to_string(),
                    bases: values.iter().filter_map(|v| real(v, "value")).collect(),
                    shown: values.first().and_then(|v| text(v, "locTag")),
                    reverse_symbol: values.first().is_some_and(|v| {
                        v.get("reverseValueSymbol").and_then(Value::as_bool) == Some(true)
                    }),
                })
            })
            .collect();
        let listed = el.get("availableChallenges").and_then(Value::as_array);
        let challenges = listed
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let mut complications: Vec<String> = c
                    .get("complications")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|m| text(m, "fullName"))
                    .collect();
                complications.sort();
                complications.dedup();
                Some((text(c, "fullName")?, complications))
            })
            .collect();
        out.push(DeRivenPool {
            unique_name: unique_name.to_string(),
            stats,
            challenges,
        });
    }
    Ok(out)
}

/// A gun mod from `ExportUpgrades`, with its top rank in both languages.
pub struct DeMod {
    pub unique_name: String,
    pub name: String,
    pub compat: String,
    pub rarity: String,
    pub polarity: String,
    pub base_drain: i64,
    pub max_rank: i64,
    /// `isUtility`: it goes in the exilus slot.
    pub exilus: bool,
    pub mod_set: Option<String>,
    pub top_en: Vec<String>,
    pub top_ru: Vec<String>,
}

/// An arcane from `ExportRelicArcane`, with its top rank in both languages.
pub struct DeArcane {
    pub unique_name: String,
    pub rarity: Option<String>,
    pub max_rank: i64,
    pub top_en: Vec<String>,
    pub top_ru: Vec<String>,
}

/// Mods for primary and secondary weapons. Rivens are left to their own reading.
pub fn de_mods(en: &[u8], ru: &[u8]) -> Result<Vec<DeMod>> {
    let doc = parse("ExportUpgrades", en)?;
    let russian = top_ranks("ExportUpgrades", ru)?;
    let mut out = Vec::new();
    for (_, el) in objects(&doc) {
        let (Some(unique_name), Some(name)) = (text(el, "uniqueName"), text(el, "name")) else {
            continue;
        };
        let gun = matches!(text(el, "type").as_deref(), Some("PRIMARY" | "SECONDARY"));
        if !gun || unique_name.contains("/Randomized/") {
            continue;
        }
        let Some(top_en) = top_rank(el) else { continue };
        out.push(DeMod {
            top_ru: russian.get(&unique_name).cloned().unwrap_or_default(),
            compat: text(el, "compatName").unwrap_or_default(),
            rarity: text(el, "rarity").unwrap_or_default(),
            polarity: text(el, "polarity").unwrap_or_default(),
            base_drain: int(el, "baseDrain").unwrap_or(0),
            max_rank: int(el, "fusionLimit").unwrap_or(0),
            exilus: el.get("isUtility").and_then(Value::as_bool) == Some(true),
            mod_set: text(el, "modSet"),
            unique_name,
            name,
            top_en,
        });
    }
    Ok(out)
}

/// Arcanes: the records of `ExportRelicArcane` that carry `levelStats`.
pub fn de_arcanes(en: &[u8], ru: &[u8]) -> Result<Vec<DeArcane>> {
    let doc = parse("ExportRelicArcane", en)?;
    let russian = top_ranks("ExportRelicArcane", ru)?;
    let mut out = Vec::new();
    for (_, el) in objects(&doc) {
        let Some(unique_name) = text(el, "uniqueName") else {
            continue;
        };
        let Some(top_en) = top_rank(el) else { continue };
        let ranks = el
            .get("levelStats")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        out.push(DeArcane {
            top_ru: russian.get(&unique_name).cloned().unwrap_or_default(),
            rarity: text(el, "rarity"),
            max_rank: ranks as i64 - 1,
            unique_name,
            top_en,
        });
    }
    Ok(out)
}

/// The lines of the last `levelStats` rank.
fn top_rank(el: &Value) -> Option<Vec<String>> {
    let ranks = el.get("levelStats").and_then(Value::as_array)?;
    let stats = ranks.last()?.get("stats").and_then(Value::as_array)?;
    Some(
        stats
            .iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect(),
    )
}

/// Every record's top rank, by item: the Russian manifest prints `levelStats` translated.
fn top_ranks(manifest: &str, raw: &[u8]) -> Result<BTreeMap<String, Vec<String>>> {
    let doc = parse(manifest, raw)?;
    Ok(objects(&doc)
        .into_iter()
        .filter_map(|(_, el)| Some((text(el, "uniqueName")?, top_rank(el)?)))
        .collect())
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

/// A recipe's ingredients, one entry per item. The export lists an item once per copy for
/// some recipes (two Kohmak for Twin Kohmak), so repeats add up.
fn ingredients(el: &Value) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = Vec::new();
    let listed = el.get("ingredients").and_then(Value::as_array);
    for ing in listed.into_iter().flatten() {
        let Some(item) = ing
            .get("ItemType")
            .and_then(Value::as_str)
            .filter(|item| !item.is_empty())
        else {
            continue;
        };
        let item = normalize::path(item).into_owned();
        let count = int(ing, "ItemCount").unwrap_or(1);
        match out.iter_mut().find(|(held, _)| *held == item) {
            Some((_, held)) => *held += count,
            None => out.push((item, count)),
        }
    }
    out
}

fn real(el: &Value, key: &str) -> Option<f64> {
    el.get(key).and_then(Value::as_f64)
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
        assert_eq!(r.output, 1);
    }

    #[test]
    fn an_ingredient_listed_once_per_copy_adds_up() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "ExportRecipes": [
                { "uniqueName": "/Lotus/Types/Recipes/Weapons/TwinKohmakBlueprint",
                  "resultType": "/Lotus/Weapons/Grineer/Pistols/GrnDWUniques/GrnTwinKohmaks",
                  "ingredients": [
                      { "ItemType": "/Lotus/Weapons/Grineer/Pistols/GrnKohmPistol/GrnKohmPistol", "ItemCount": 1 },
                      { "ItemType": "/Lotus/Weapons/Grineer/Pistols/GrnKohmPistol/GrnKohmPistol", "ItemCount": 1 },
                      { "ItemType": "/Lotus/Types/Items/MiscItems/Forma", "ItemCount": 1 }
                  ] }
            ]
        }))
        .unwrap();
        assert_eq!(
            de_recipes(&raw).unwrap()[0].ingredients,
            vec![
                (
                    "/Lotus/Weapons/Grineer/Pistols/GrnKohmPistol/GrnKohmPistol".to_string(),
                    2
                ),
                ("/Lotus/Types/Items/MiscItems/Forma".to_string(), 1),
            ]
        );
    }

    #[test]
    fn a_recipe_says_how_many_copies_one_build_hands_over() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "ExportRecipes": [
                { "uniqueName": "/Lotus/Types/Recipes/DeimosRecipes/Prospecting/DeimosCommonOreAAlloyBlueprint",
                  "resultType": "/Lotus/Types/Items/Gems/Deimos/DeimosCommonOreAAlloyItem",
                  "num": 20, "ingredients": [] }
            ]
        }))
        .unwrap();
        assert_eq!(de_recipes(&raw).unwrap()[0].output, 20);
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

    #[test]
    fn reads_the_rank_an_item_asks_and_how_far_it_ranks() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "ExportWeapons": [
                { "uniqueName": "/Lotus/Weapons/Tenno/Pistol/Torid", "name": "Torid",
                  "productCategory": "LongGuns", "masteryReq": 4 },
                { "uniqueName": "/Lotus/Weapons/Grineer/KuvaLich/Secondaries/Nukor/KuvaNukor",
                  "name": "Kuva Nukor", "productCategory": "Pistols",
                  "masteryReq": 13, "maxLevelCap": 40 }
            ]
        }))
        .unwrap();
        let items = de_items("ExportWeapons", &raw).unwrap();
        assert_eq!(
            (items[0].mastery_req, items[0].max_level_cap),
            (Some(4), None)
        );
        assert_eq!(
            (items[1].mastery_req, items[1].max_level_cap),
            (Some(13), Some(40))
        );
    }

    #[test]
    fn reads_a_weapons_disposition_and_printed_attack() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "ExportWeapons": [
                { "uniqueName": "/Lotus/Weapons/ClanTech/Bio/BioWeapon", "name": "Torid",
                  "damagePerShot": [0, 0, 0, 0, 0, 0, 140, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                  "totalDamage": 140, "criticalChance": 0.15000001, "criticalMultiplier": 2,
                  "procChance": 0.23000002, "fireRate": 1.5000001, "omegaAttenuation": 1.3,
                  "trigger": "SEMI", "magazineSize": 5, "reloadTime": 1.7, "multishot": 1 },
                { "uniqueName": "/Lotus/Weapons/Ostron/Melee/ModularMelee01/Tip/TipOne",
                  "name": "Balla",
                  "damagePerShot": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0.5, 0, 5.8e25, 0, 0, 0, 0, 0, 0, 0],
                  "totalDamage": 0, "criticalChance": 0, "criticalMultiplier": 0,
                  "procChance": 0, "fireRate": 0, "omegaAttenuation": 0.8 }
            ],
            "ExportRailjackWeapons": [
                { "uniqueName": "/Lotus/Weapons/CrewShip/Pulsar", "name": "Pulsar",
                  "omegaAttenuation": 1 }
            ]
        }))
        .unwrap();
        let weapons = de_weapons(&raw).unwrap();
        assert_eq!(weapons.len(), 2);

        let torid = &weapons["/Lotus/Weapons/ClanTech/Bio/BioWeapon"];
        assert_eq!(torid.disposition, 1.3);
        let stats = torid.stats.as_ref().unwrap();
        assert_eq!(stats.damage, vec![(Damage::Toxin, 140.0)]);
        assert_eq!(stats.trigger.as_deref(), Some("SEMI"));
        assert_eq!((stats.magazine, stats.multishot), (Some(5), Some(1)));

        let tip = &weapons["/Lotus/Weapons/Ostron/Melee/ModularMelee01/Tip/TipOne"];
        assert_eq!((tip.disposition, tip.stats.is_none()), (0.8, true));
    }

    #[test]
    fn reads_a_gun_mods_top_rank_in_both_languages() {
        let en = serde_json::to_vec(&serde_json::json!({
            "ExportUpgrades": [
                { "uniqueName": "/Lotus/Upgrades/Mods/Rifle/WeaponDamageAmountMod",
                  "name": "Serration", "polarity": "AP_ATTACK", "rarity": "UNCOMMON",
                  "baseDrain": 4, "fusionLimit": 10, "compatName": "Rifle", "type": "PRIMARY",
                  "levelStats": [ { "stats": ["+15% Damage"] }, { "stats": ["+165% Damage"] } ] },
                { "uniqueName": "/Lotus/Upgrades/Mods/Warframe/AvatarAbilityStrengthMod",
                  "name": "Intensify", "type": "WARFRAME",
                  "levelStats": [ { "stats": ["+30% Ability Strength"] } ] }
            ]
        }))
        .unwrap();
        let ru = serde_json::to_vec(&serde_json::json!({
            "ExportUpgrades": [
                { "uniqueName": "/Lotus/Upgrades/Mods/Rifle/WeaponDamageAmountMod",
                  "name": "Зазубрина",
                  "levelStats": [ { "stats": ["+15% урона"] }, { "stats": ["+165% урона"] } ] }
            ]
        }))
        .unwrap();
        let mods = de_mods(&en, &ru).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].top_en, vec!["+165% Damage"]);
        assert_eq!(mods[0].top_ru, vec!["+165% урона"]);
        assert_eq!((mods[0].max_rank, mods[0].base_drain), (10, 4));
    }
}
