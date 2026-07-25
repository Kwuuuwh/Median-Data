use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{DeItem, DeRecipe};
use crate::rules;

/// Catalog paths a source can name, keyed by their English name. Blueprints DE declares
/// only as recipe keys are included, since the market trades them by name.
pub struct Paths {
    by_name: BTreeMap<String, Vec<String>>,
    known: BTreeSet<String>,
}

impl Paths {
    pub fn build(de: &[DeItem], recipes: &[DeRecipe]) -> Self {
        let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut known = BTreeSet::new();
        let mut de_names = BTreeMap::new();

        for item in de {
            known.insert(item.unique_name.clone());
            de_names.insert(item.unique_name.as_str(), item.name.as_str());
            by_name
                .entry(normalize(&item.name))
                .or_default()
                .push(item.unique_name.clone());
        }

        for r in recipes {
            if known.contains(&r.blueprint) {
                continue;
            }
            let Some(result) = de_names.get(r.result.as_str()) else {
                continue;
            };
            known.insert(r.blueprint.clone());
            by_name
                .entry(normalize(&format!("{result} Blueprint")))
                .or_default()
                .push(r.blueprint.clone());
        }

        for paths in by_name.values_mut() {
            paths.sort();
            paths.dedup();
        }
        Self { by_name, known }
    }

    pub fn has(&self, path: &str) -> bool {
        self.known.contains(path)
    }

    /// The one path answering to a name, once the two kinds of duplicate DE keeps are
    /// resolved: a relic's four refinements are one relic, and a tier folder holds copies
    /// of items that also exist untiered.
    pub fn one(&self, name: &str) -> Option<&str> {
        let candidates = self.by_name.get(&normalize(name))?;
        let narrowed = narrow(candidates);
        match narrowed.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }
}

fn narrow(candidates: &[String]) -> Vec<&str> {
    let mut out: Vec<&str> = candidates.iter().map(String::as_str).collect();
    if out.iter().all(|p| rules::relic(p).is_some()) {
        out.retain(|p| rules::relic(p).is_some_and(|r| r.refinement == "intact"));
    } else if out.iter().any(|p| !graph::tiered(p)) {
        out.retain(|p| !graph::tiered(p));
    }
    out
}

fn normalize(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(unique_name: &str, name: &str) -> DeItem {
        DeItem {
            unique_name: unique_name.into(),
            name: name.into(),
            category: "Test".into(),
            array: "Test".into(),
            parent: None,
            mod_type: None,
        }
    }

    fn recipe(blueprint: &str, result: &str) -> DeRecipe {
        DeRecipe {
            blueprint: blueprint.into(),
            result: result.into(),
            ingredients: Vec::new(),
            build_price: None,
            build_time: None,
            consumed: true,
            rush_price: None,
        }
    }

    #[test]
    fn a_recipe_key_is_named_after_what_it_builds() {
        let de = vec![item("/Powersuits/VoltPrime", "Volt Prime")];
        let recipes = vec![recipe("/Recipes/VoltPrimeBlueprint", "/Powersuits/VoltPrime")];
        let paths = Paths::build(&de, &recipes);
        assert_eq!(
            paths.one("Volt Prime Blueprint"),
            Some("/Recipes/VoltPrimeBlueprint")
        );
        assert!(paths.has("/Recipes/VoltPrimeBlueprint"));
    }

    #[test]
    fn a_relic_resolves_to_its_intact_refinement() {
        let de = vec![
            item("/Game/Projections/T1VoidProjectionA", "Lith A1 Relic"),
            item("/Game/Projections/T1VoidProjectionAGold", "Lith A1 Relic"),
        ];
        let paths = Paths::build(&de, &[]);
        assert_eq!(
            paths.one("Lith A1 Relic"),
            Some("/Game/Projections/T1VoidProjectionA")
        );
    }

    #[test]
    fn a_tier_copy_loses_to_the_real_item() {
        let de = vec![
            item("/Mods/Beginner/Redirection", "Redirection"),
            item("/Mods/Redirection", "Redirection"),
        ];
        let paths = Paths::build(&de, &[]);
        assert_eq!(paths.one("Redirection"), Some("/Mods/Redirection"));
    }

    #[test]
    fn a_genuinely_shared_name_resolves_to_nothing() {
        let de = vec![item("/A/Sunder", "Sunder"), item("/B/Sunder", "Sunder")];
        let paths = Paths::build(&de, &[]);
        assert_eq!(paths.one("Sunder"), None);
    }
}
