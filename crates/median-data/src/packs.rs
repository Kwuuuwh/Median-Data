use std::collections::{BTreeMap, BTreeSet};

use graph::{Edge, Graph, Rel};
use serde::{Deserialize, Serialize};
use sources::notation::Value;

use crate::extract::DeItem;
use crate::lineage::{Lineage, resolve};
use crate::normalize;

/// Where the game keeps the packs a stall sells.
const PACKS: &str = "/Lotus/StoreItems/Types/BoosterPacks/";

/// Where the game keeps arcanes.
const ARCANES: &str = "/Upgrades/CosmeticEnhancers/";

/// What DE files a pack under, as the catalog sees it.
const CATEGORY: &str = "BoosterPacks";

/// The arcane collections the stalls sell, in the game's own paths.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Packs {
    #[serde(default)]
    pub pack: Vec<Pack>,
}

/// A pack and the arcanes it rolls from.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Pack {
    /// The store path stalls sell it under.
    pub item: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_ru: Option<String>,
    pub rolls: Vec<String>,
}

/// The client's phrases in one language, keyed by their localisation tag.
pub type Phrases = BTreeMap<String, String>;

/// Every pack a stall sells whose rolls are all arcanes, in path order.
pub fn read(lineage: &Lineage, en: &Phrases, ru: &Phrases, sold: &BTreeSet<&str>) -> Packs {
    let mut pack: Vec<Pack> = lineage
        .paths()
        .filter(|path| path.starts_with(PACKS) && sold.contains(path))
        .filter_map(|path| collection(lineage, path, en, ru))
        .collect();
    pack.sort_by(|one, other| one.item.cmp(&other.item));
    Packs { pack }
}

fn collection(lineage: &Lineage, store: &str, en: &Phrases, ru: &Phrases) -> Option<Pack> {
    let (_, Value::Text(kind)) = lineage.field(store, "TypeName")? else {
        return None;
    };
    let (owner, Value::Block(listed)) = lineage.field(&kind, "Components")? else {
        return None;
    };
    let mut rolls: Vec<String> = listed
        .items
        .iter()
        .filter_map(Value::block)
        .filter_map(|component| component.text("Item"))
        .map(|item| resolve(owner, item))
        .collect();
    rolls.sort();
    rolls.dedup();
    if rolls.is_empty() || !rolls.iter().all(|item| item.contains(ARCANES)) {
        return None;
    }
    let tag = match lineage.field(store, "LocalizeTag") {
        Some((_, Value::Text(tag))) => tag,
        _ => String::new(),
    };
    Some(Pack {
        item: store.to_string(),
        name: en.get(&tag).cloned(),
        name_ru: ru.get(&tag).cloned(),
        rolls,
    })
}

/// The packs as items, for the ones the client names, with their Russian names beside.
pub fn items(packs: &Packs) -> Vec<(DeItem, Option<String>)> {
    packs
        .pack
        .iter()
        .filter_map(|pack| {
            let item = DeItem {
                unique_name: normalize::path(&pack.item).into_owned(),
                name: pack.name.clone()?,
                category: CATEGORY.to_string(),
                array: CATEGORY.to_string(),
                parent: None,
                mod_type: None,
                mastery_req: None,
                max_level_cap: None,
            };
            Some((item, pack.name_ru.clone()))
        })
        .collect()
}

/// An edge from every pack to each arcane it rolls that the catalog holds. Says how many it drew.
pub fn link(graph: &mut Graph, packs: &Packs) -> usize {
    let mut drawn = 0;
    for pack in &packs.pack {
        let owner = normalize::path(&pack.item).into_owned();
        if !graph.has(&owner) {
            continue;
        }
        for roll in &pack.rolls {
            let item = normalize::path(roll).into_owned();
            if !graph.has(&item) {
                continue;
            }
            graph.link(Edge {
                from: owner.clone(),
                to: item,
                rel: Rel::Grants(graph::Grant::Pooled),
            });
            drawn += 1;
        }
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineage::tests::kind;

    fn phrases(pairs: &[(&str, &str)]) -> Phrases {
        pairs
            .iter()
            .map(|(tag, text)| (tag.to_string(), text.to_string()))
            .collect()
    }

    #[test]
    fn a_pack_no_stall_sells_is_not_a_collection() {
        let lineage = Lineage::new(vec![
            kind(
                "/Lotus/StoreItems/Types/BoosterPacks/CalendarArtifactPack",
                None,
                Some("TypeName=/Lotus/Types/BoosterPacks/CalendarArtifactPack\n"),
            ),
            kind(
                "/Lotus/Types/BoosterPacks/CalendarArtifactPack",
                None,
                Some("Components={\n{\nItem=/Lotus/Upgrades/CosmeticEnhancers/Offensive/A\n}\n}\n"),
            ),
        ]);
        let none = Phrases::new();

        assert!(
            read(&lineage, &none, &none, &BTreeSet::new())
                .pack
                .is_empty()
        );
    }

    #[test]
    fn a_store_pack_rolls_the_arcanes_its_type_lists() {
        let lineage = Lineage::new(vec![
            kind(
                "/Lotus/StoreItems/Types/BoosterPacks/ArcanePackHex",
                None,
                Some(
                    "TypeName=/Lotus/Types/BoosterPacks/ArcanePackHex\n\
                     LocalizeTag=/Lotus/Language/ArcanePackHex\n",
                ),
            ),
            kind(
                "/Lotus/Types/BoosterPacks/ArcanePackHex",
                None,
                Some(
                    "Components={\n{\nItem=/Lotus/Upgrades/CosmeticEnhancers/Offensive/B\nRarity=RARE\n},\n\
                     {\nItem=/Lotus/Upgrades/CosmeticEnhancers/Offensive/A\nRarity=RARE\n}\n}\n",
                ),
            ),
            kind(
                "/Lotus/StoreItems/Types/BoosterPacks/ModPack",
                None,
                Some("TypeName=/Lotus/Types/BoosterPacks/ModPack\n"),
            ),
            kind(
                "/Lotus/Types/BoosterPacks/ModPack",
                None,
                Some("Components={\n{\nItem=/Lotus/Upgrades/Mods/Rifle/Serration\n}\n}\n"),
            ),
        ]);
        let en = phrases(&[("/Lotus/Language/ArcanePackHex", "Hex Arcane Collection")]);
        let ru = phrases(&[("/Lotus/Language/ArcanePackHex", "Коллекция: Гекс")]);

        let sold = BTreeSet::from([
            "/Lotus/StoreItems/Types/BoosterPacks/ArcanePackHex",
            "/Lotus/StoreItems/Types/BoosterPacks/ModPack",
        ]);

        assert_eq!(
            read(&lineage, &en, &ru, &sold).pack,
            [Pack {
                item: "/Lotus/StoreItems/Types/BoosterPacks/ArcanePackHex".into(),
                name: Some("Hex Arcane Collection".into()),
                name_ru: Some("Коллекция: Гекс".into()),
                rolls: vec![
                    "/Lotus/Upgrades/CosmeticEnhancers/Offensive/A".into(),
                    "/Lotus/Upgrades/CosmeticEnhancers/Offensive/B".into(),
                ],
            }]
        );
    }
}
