use std::collections::BTreeMap;

use graph::{DropInfo, Edge, Graph, Node, Place, PlaceKind, Rel, place_id, printed};
use serde::{Deserialize, Serialize};
use sources::notation::{Block, Value};

use crate::lineage::{Lineage, resolve};
use crate::normalize;

const STORE: &str = "/Lotus/StoreItems/";

/// What the incubator takes and what it can hatch, in the game's own paths.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Incubator {
    #[serde(default)]
    pub egg: Vec<Egg>,
    #[serde(default)]
    pub recipe: Vec<Recipe>,
}

/// An egg and the breeds it can hatch into when nothing steers it.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Egg {
    pub item: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_ru: Option<String>,
    pub pool: Vec<Pool>,
}

/// The client's phrases in one language, keyed by their localisation tag.
pub type Phrases = BTreeMap<String, String>;

/// Breeds the egg lists under one weight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pool {
    pub weight: i64,
    pub breeds: Vec<String>,
}

/// One way to start an incubation.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    pub recipe: String,
    /// Seconds the incubation takes.
    pub seconds: i64,
    pub ingredient: Vec<Ingredient>,
    /// Breeds an imprint can steer the incubation toward.
    pub templates: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Ingredient {
    pub item: String,
    pub count: i64,
}

/// Every ownable egg with a breed pool, and every recipe that hatches a pet, each once
/// under its own path rather than its store one.
pub fn read(lineage: &Lineage, en: &Phrases, ru: &Phrases) -> Incubator {
    let mut incubator = Incubator::default();
    for path in lineage.paths() {
        if lineage.ownable(path) {
            if let Some(egg) = egg(lineage, path, en, ru) {
                incubator.egg.push(egg);
            }
        }
        if let Some(recipe) = recipe(lineage, path) {
            incubator.recipe.push(recipe);
        }
    }
    incubator
        .egg
        .sort_by(|one, other| one.item.cmp(&other.item));
    incubator
        .recipe
        .sort_by(|one, other| one.recipe.cmp(&other.recipe));
    incubator
}

/// A place for every breed pool an egg hatches from, named after the egg, dropping each
/// breed by its share of the pool. Says how many drops it drew.
pub fn link(graph: &mut Graph, incubator: &Incubator) -> usize {
    let mut drawn = 0;
    let mut seen: Vec<&Vec<Pool>> = Vec::new();
    for egg in &incubator.egg {
        let Some(name) = &egg.name else {
            continue;
        };
        if seen.contains(&&egg.pool) {
            continue;
        }
        seen.push(&egg.pool);
        graph.insert(Node::Place(Place {
            name: name.clone(),
            name_ru: egg.name_ru.clone(),
            kind: PlaceKind::Incubator,
            bounty: None,
            table: None,
        }));
        let total: i64 = egg.pool.iter().map(|pool| pool.weight).sum();
        for pool in &egg.pool {
            for breed in &pool.breeds {
                let item = normalize::path(breed).into_owned();
                if total <= 0 || !graph.has(&item) {
                    continue;
                }
                let chance = pool.weight as f64 / total as f64 / pool.breeds.len() as f64;
                graph.link(Edge {
                    from: place_id(name),
                    to: item,
                    rel: Rel::Drops(DropInfo {
                        rarity: printed(chance).to_string(),
                        chance,
                        rotation: None,
                        stage: None,
                        table_chance: None,
                        levels: None,
                        count: None,
                    }),
                });
                drawn += 1;
            }
        }
    }
    drawn
}

fn egg(lineage: &Lineage, path: &str, en: &Phrases, ru: &Phrases) -> Option<Egg> {
    let (owner, Value::Block(listed)) = lineage.field(path, "Personalities")? else {
        return None;
    };
    let pool: Vec<Pool> = blocks(&listed)
        .filter_map(|pool| {
            let breeds = pool
                .block("Values")?
                .items
                .iter()
                .filter_map(Value::text)
                .map(|breed| resolve(owner, breed))
                .collect();
            Some(Pool {
                weight: pool.int("Weight").unwrap_or(1),
                breeds,
            })
        })
        .collect();
    let tag = match lineage.field(path, "LocalizeTag") {
        Some((_, Value::Text(tag))) => tag,
        _ => String::new(),
    };
    (!pool.is_empty()).then(|| Egg {
        item: path.to_string(),
        name: en.get(&tag).cloned(),
        name_ru: ru.get(&tag).cloned(),
        pool,
    })
}

fn recipe(lineage: &Lineage, path: &str) -> Option<Recipe> {
    if path.starts_with(STORE) || !lineage.sets(path, "SecretIngredientAction") {
        return None;
    }
    let own = lineage.own(path)?;
    if own.text("SecretIngredientAction") != Some("SIA_CREATE_KUBROW") {
        return None;
    }
    let items = |key: &str| -> Vec<Ingredient> {
        own.block(key)
            .map(|listed| {
                blocks(listed)
                    .filter_map(|line| {
                        Some(Ingredient {
                            item: resolve(path, line.text("ItemType")?),
                            count: line.int("ItemCount").unwrap_or(1),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(Recipe {
        recipe: path.to_string(),
        seconds: own.int("BuildTime").unwrap_or(0),
        ingredient: items("Ingredients"),
        templates: items("SecretIngredients")
            .into_iter()
            .map(|breed| breed.item)
            .collect(),
    })
}

fn blocks(listed: &Block) -> impl Iterator<Item = &Block> {
    listed.items.iter().filter_map(Value::block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineage::tests::kind;
    use consensus::{Claim, Resolved, Source, resolve};
    use graph::{Extra, Item, Names};

    fn value<T: Clone + PartialEq>(v: T) -> Resolved<T> {
        resolve(
            &[Claim {
                source: Source::De,
                value: v,
            }],
            &[Source::De],
        )
        .unwrap()
    }

    fn item(path: &str, name: &str) -> Node {
        Node::Item(Item {
            unique_name: path.into(),
            names: Names {
                en: value(name.to_string()),
                ru: None,
            },
            category: value("KubrowPets".to_string()),
            kind: value(graph::Kind::unknown()),
            slug: None,
            tradable: None,
            vaulted: None,
            prime: value(false),
            ducats: None,
            mastery: None,
            mastery_req: None,
            max_level_cap: None,
            extra: Extra::None,
        })
    }

    const TAG: &str = "/Lotus/Language/Items/CatbrowEggName";

    const EGG: &str = "Personalities={
{
Values={
/Lotus/Types/Game/CatbrowPet/MirrorCatbrowPetPowerSuit,
/Lotus/Types/Game/CatbrowPet/CheshireCatbrowPetPowerSuit
}
Weight=1
}
}
";

    const RECIPE: &str =
        "ResultItem=/Lotus/StoreItems/Types/Game/KubrowPet/AdventurerKubrowPetPowerSuit
Ingredients={
{
ItemType=Eggs/KubrowEgg
ItemCount=1
},
{
ItemType=EggHatcher
ItemCount=1
}
}
BuildTime=172800
SecretIngredientAction=SIA_CREATE_KUBROW
SecretIngredients={
{
ItemType=AdventurerKubrowPetPowerSuit
ItemCount=1
}
}
";

    #[test]
    fn an_egg_inherits_the_breeds_it_can_hatch() {
        let lineage = Lineage::new(vec![
            kind(
                "/Lotus/Types/Game/CatbrowPet/Eggs/CatbrowPetEggItem",
                None,
                Some(format!("LocalizeTag={TAG}\n{EGG}").as_str()),
            ),
            kind(
                "/Lotus/Types/Game/CatbrowPet/Eggs/CatbrowEgg",
                Some("/Lotus/Types/Game/CatbrowPet/Eggs/CatbrowPetEggItem"),
                None,
            ),
            kind(
                "/Lotus/StoreItems/Types/Game/CatbrowPet/Eggs/CatbrowEgg",
                None,
                None,
            ),
        ]);

        let en = Phrases::from([(TAG.to_string(), "Kavat Egg".to_string())]);
        let ru = Phrases::from([(TAG.to_string(), "Яйцо кавата".to_string())]);

        let incubator = read(&lineage, &en, &ru);

        assert_eq!(
            incubator.egg,
            [Egg {
                item: "/Lotus/Types/Game/CatbrowPet/Eggs/CatbrowEgg".into(),
                name: Some("Kavat Egg".into()),
                name_ru: Some("Яйцо кавата".into()),
                pool: vec![Pool {
                    weight: 1,
                    breeds: vec![
                        "/Lotus/Types/Game/CatbrowPet/MirrorCatbrowPetPowerSuit".into(),
                        "/Lotus/Types/Game/CatbrowPet/CheshireCatbrowPetPowerSuit".into(),
                    ],
                }],
            }]
        );
    }

    #[test]
    fn each_breed_drops_by_its_share_of_the_pool() {
        let mut graph = Graph::new();
        for (path, name) in [
            (
                "/Lotus/Types/Game/CatbrowPet/MirrorCatbrowPetPowerSuit",
                "Adarza",
            ),
            (
                "/Lotus/Types/Game/CatbrowPet/CheshireCatbrowPetPowerSuit",
                "Smeeta",
            ),
        ] {
            graph.insert(item(path, name));
        }
        let pool = vec![Pool {
            weight: 1,
            breeds: vec![
                "/Lotus/Types/Game/CatbrowPet/MirrorCatbrowPetPowerSuit".into(),
                "/Lotus/Types/Game/CatbrowPet/CheshireCatbrowPetPowerSuit".into(),
            ],
        }];
        let egg = |item: &str| Egg {
            item: item.into(),
            name: Some("Kavat Gene-Matter".into()),
            name_ru: None,
            pool: pool.clone(),
        };
        let incubator = Incubator {
            egg: vec![
                egg("/Lotus/Types/Game/CatbrowPet/Eggs/CatbrowEgg"),
                egg("/Lotus/Types/Game/CatbrowPet/Eggs/CatbrowPetEggItem"),
            ],
            recipe: Vec::new(),
        };

        assert_eq!(link(&mut graph, &incubator), 2);
        let chances: Vec<f64> = graph
            .from(&place_id("Kavat Gene-Matter"))
            .iter()
            .filter_map(|edge| match &edge.rel {
                Rel::Drops(drop) => Some(drop.chance),
                _ => None,
            })
            .collect();
        assert_eq!(chances, [0.5, 0.5]);
    }

    #[test]
    fn a_hatching_recipe_names_its_ingredients_and_templates_in_full() {
        let lineage = Lineage::new(vec![kind(
            "/Lotus/Types/Game/KubrowPet/KubrowPetRecipe",
            None,
            Some(RECIPE),
        )]);

        let recipe = &read(&lineage, &Phrases::new(), &Phrases::new()).recipe[0];

        assert_eq!(recipe.seconds, 172800);
        assert_eq!(
            recipe.ingredient[0].item,
            "/Lotus/Types/Game/KubrowPet/Eggs/KubrowEgg"
        );
        assert_eq!(
            recipe.templates,
            ["/Lotus/Types/Game/KubrowPet/AdventurerKubrowPetPowerSuit"]
        );
    }
}
