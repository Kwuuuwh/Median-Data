use std::collections::BTreeSet;

use graph::{RelicInfo, Table};

use crate::extract::{DeRecipe, DeReward};

/// A prime-line item carries "Prime" as a whole word in its English name. Names alone are
/// weak evidence: decorations depicting a prime carry the word too.
pub fn is_prime(name: &str) -> bool {
    name.split_whitespace().any(|w| w == "Prime")
}

/// What the void economy hands out: everything relics award, plus whatever is assembled
/// from those parts. Relics also award Forma, so this is a place, not a lineage.
pub fn void_economy(rewards: &[DeReward], recipes: &[DeRecipe]) -> BTreeSet<String> {
    let mut inside: BTreeSet<String> = rewards.iter().map(|r| r.reward.clone()).collect();
    for r in recipes {
        let built_from_inside = inside.contains(&r.blueprint)
            || r.ingredients.iter().any(|(item, _)| inside.contains(item));
        if built_from_inside {
            inside.insert(r.result.clone());
        }
    }
    inside
}

/// A prime-line item is named Prime *and* sits in the void economy. The name alone marks
/// decorations depicting a prime; the economy alone marks Forma.
pub fn prime(name: &str, path: &str, economy: &BTreeSet<String>) -> bool {
    is_prime(name) && economy.contains(path)
}

/// DE grade suffixes, in refinement order.
const GRADES: &[(&str, &str)] = &[
    ("Bronze", "intact"),
    ("Silver", "exceptional"),
    ("Gold", "flawless"),
    ("Platinum", "radiant"),
];

/// Split a void relic path into its logical relic and refinement. Relics live under
/// `/Projections/`, one entity per refinement.
pub fn relic(unique_name: &str) -> Option<RelicInfo> {
    if !unique_name.contains("/Projections/") {
        return None;
    }
    let (base, refinement) = GRADES
        .iter()
        .find_map(|(suffix, refinement)| {
            unique_name
                .strip_suffix(suffix)
                .map(|base| (base.to_string(), (*refinement).to_string()))
        })
        .unwrap_or_else(|| (unique_name.to_string(), "intact".to_string()));
    Some(RelicInfo {
        base,
        refinement,
        vaulted_in: None,
    })
}

/// Read a node's reward-table heading as the drop tables print it: `Location/Node (Label)`,
/// with an `Event:` prefix for a past event and a trailing `Extra` for the node's second
/// table. Anything not shaped that way names no node.
pub fn heading(printed: &str) -> Option<Table> {
    let event = printed.starts_with("Event:");
    let rest = printed.strip_prefix("Event:").unwrap_or(printed).trim();
    let extra = rest.ends_with("Extra");
    let rest = rest.strip_suffix("Extra").unwrap_or(rest).trim_end();
    let close = rest.strip_suffix(')')?;
    let (place, label) = close.rsplit_once('(')?;
    let (location, node) = place.trim_end().split_once('/')?;
    Some(Table {
        location: location.trim().to_string(),
        node: node.trim().to_string(),
        label: label.trim().to_string(),
        extra,
        event,
    })
}

/// Items assembled from a spent blueprint. The game trades the blueprint, never the
/// built result, whatever a market lists.
pub fn built(recipes: &[DeRecipe]) -> BTreeSet<String> {
    recipes
        .iter()
        .filter(|r| r.consumed)
        .map(|r| r.result.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(result: &str, consumed: bool) -> DeRecipe {
        DeRecipe {
            blueprint: format!("{result}Blueprint"),
            result: result.into(),
            ingredients: Vec::new(),
            build_price: None,
            build_time: None,
            consumed,
            rush_price: None,
        }
    }

    #[test]
    fn prime_needs_a_whole_word() {
        assert!(is_prime("Volt Prime"));
        assert!(!is_prime("Primed Continuity"));
    }

    #[test]
    fn prime_needs_both_the_name_and_the_economy() {
        let economy = ["/R/VoltPrimeChassisBlueprint", "/R/FormaBlueprint"]
            .into_iter()
            .map(String::from)
            .collect();
        assert!(prime(
            "Volt Prime Chassis Blueprint",
            "/R/VoltPrimeChassisBlueprint",
            &economy
        ));
        // a relic awards Forma, which is not a prime
        assert!(!prime("Forma Blueprint", "/R/FormaBlueprint", &economy));
        // a decoration depicting a prime is not one
        assert!(!prime(
            "Noggle Statue - Ash Prime",
            "/D/AshPrimeBobbleHead",
            &economy
        ));
    }

    #[test]
    fn economy_spreads_through_recipes() {
        let rewards = vec![DeReward {
            relic: "/Relic/A".into(),
            reward: "/R/VoltPrimeBlueprint".into(),
            rarity: "RARE".into(),
        }];
        let recipes = vec![DeRecipe {
            blueprint: "/R/VoltPrimeBlueprint".into(),
            result: "/Powersuits/VoltPrime".into(),
            ingredients: Vec::new(),
            build_price: None,
            build_time: None,
            consumed: true,
            rush_price: None,
        }];
        let economy = void_economy(&rewards, &recipes);
        assert!(economy.contains("/Powersuits/VoltPrime"));
    }

    #[test]
    fn built_results_come_from_consumed_recipes() {
        let recipes = vec![
            recipe("/R/VoltPrimeChassisComponent", true),
            recipe("/M/AlloyPlate", false),
        ];
        let built = built(&recipes);
        assert!(built.contains("/R/VoltPrimeChassisComponent"));
        assert!(!built.contains("/M/AlloyPlate"));
    }

    #[test]
    fn relic_grade_maps_to_refinement() {
        let gold = relic("/Lotus/Types/Game/Projections/T2VoidProjectionNGold").unwrap();
        assert_eq!(gold.base, "/Lotus/Types/Game/Projections/T2VoidProjectionN");
        assert_eq!(gold.refinement, "flawless");
        assert!(relic("/Lotus/Powersuits/Volt/VoltPrime").is_none());
    }

    #[test]
    fn relic_without_grade_is_intact() {
        let plain = relic("/Lotus/Types/Game/Projections/Oddity").unwrap();
        assert_eq!(plain.refinement, "intact");
    }

    #[test]
    fn reads_a_heading_into_its_parts() {
        let h = heading("Saturn/Anthe (Rescue)").unwrap();
        assert_eq!(
            (h.location.as_str(), h.node.as_str(), h.label.as_str()),
            ("Saturn", "Anthe", "Rescue")
        );
        assert!(!h.extra && !h.event);
    }

    #[test]
    fn marks_the_event_prefix_and_the_extra_suffix() {
        let h = heading("Event: Uranus/Miranda (Defense)").unwrap();
        assert!(h.event && h.node == "Miranda");
        let h = heading("Ceres/Exta (Assassination) Extra").unwrap();
        assert!(h.extra && h.label == "Assassination");
    }

    #[test]
    fn a_heading_keeps_a_node_name_holding_its_own_brackets() {
        assert_eq!(
            heading("Uranus/Scoria's Angel (Skirmish)").unwrap().node,
            "Scoria's Angel"
        );
    }

    #[test]
    fn an_enemy_name_is_no_heading() {
        assert!(heading("Grineer Lancer").is_none());
        assert!(heading("Orokin Derelict Defense").is_none());
    }
}
