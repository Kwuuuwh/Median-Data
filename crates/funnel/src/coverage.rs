use std::collections::BTreeSet;

use graph::{Graph, Node, Rel};

use crate::finding::{Finding, Layer};

/// The check that fires on a drop-table name no item answers to. Named because a decision
/// that such a name is not an item at all has to silence exactly this check.
pub const DROP_NOT_IN_CATALOG: &str = "drop-not-in-catalog";

/// What the sources named while the graph was being built but nothing could be attached to.
/// These never reach the graph, so the funnel has to be told about them.
#[derive(Default)]
pub struct Gaps {
    /// Relic rewards naming a path the catalog does not hold.
    pub unresolved_rewards: Vec<String>,
    /// Recipe endpoints the catalog does not hold.
    pub dangling_craft: Vec<String>,
    /// Drop-table rewards whose printed name matches no item.
    pub unknown_drop_items: Vec<String>,
    /// Item paths judged to keep the name the game writes, so no Russian is owed.
    pub verbatim: BTreeSet<String>,
}

/// Find what the catalog is missing, both from the build's own gaps and from the graph.
pub fn check(graph: &Graph, gaps: &Gaps) -> Vec<Finding> {
    let mut out = Vec::new();

    for path in &gaps.unresolved_rewards {
        out.push(Finding::new(
            Layer::Coverage,
            "reward-not-in-catalog",
            path,
            "awarded by a relic but absent".to_string(),
        ));
    }
    for path in &gaps.dangling_craft {
        out.push(Finding::new(
            Layer::Coverage,
            "craft-not-in-catalog",
            path,
            "named by a recipe but absent".to_string(),
        ));
    }
    for name in &gaps.unknown_drop_items {
        out.push(Finding::new(
            Layer::Coverage,
            DROP_NOT_IN_CATALOG,
            name,
            "dropped somewhere but matches no item".to_string(),
        ));
    }

    for item in graph.items() {
        if item.kind.value.is_unknown() {
            out.push(Finding::new(
                Layer::Coverage,
                "kind-unresolved",
                &item.unique_name,
                format!(
                    "no taxonomy rule matched, DE files it as {}",
                    item.category.value
                ),
            ));
        }
        if item.names.ru.is_none() && !gaps.verbatim.contains(&item.unique_name) {
            out.push(Finding::new(
                Layer::Coverage,
                "russian-name-missing",
                &item.unique_name,
                item.names.en.value.clone(),
            ));
        }
        // A prime nobody can obtain means its relics or recipe went missing.
        if item.prime.value && !graph::obtainable(graph, &item.unique_name) {
            out.push(Finding::new(
                Layer::Coverage,
                "prime-unobtainable",
                &item.unique_name,
                "no recipe, relic or drop leads to it".to_string(),
            ));
        }
    }

    out.extend(dead_recipes(graph));

    for node in graph.nodes() {
        if let Node::Set(set) = node
            && !graph
                .from(&node.id())
                .iter()
                .any(|e| matches!(e.rel, Rel::Represents))
        {
            out.push(Finding::new(
                Layer::Coverage,
                "set-without-item",
                &set.slug,
                "traded as a set but assembles nothing in the catalog".to_string(),
            ));
        }
    }

    out
}

/// Whether the item can simply be farmed: something drops it or a relic awards it. A vendor
/// selling it is deliberately not enough — a vendor who sells the finished thing usually sells
/// its blueprint too, and the source we read may only list one of the two.
fn farmed(graph: &Graph, item: &str) -> bool {
    graph
        .into(item)
        .iter()
        .any(|e| matches!(e.rel, Rel::Drops(_) | Rel::Rewards { .. }))
}

/// Recipes no player can start: nothing hands over the blueprint, while the thing it builds can
/// be farmed anyway. DE has shipped such leftovers for years — a recipe in the export that the
/// foundry never offers.
fn dead_recipes(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    for node in graph.nodes() {
        let Node::Recipe(recipe) = node else { continue };
        let Some(result) = graph
            .from(&node.id())
            .into_iter()
            .find(|e| e.rel == Rel::Produces)
            .map(|e| e.to.clone())
        else {
            continue;
        };
        if graph::handed_over(graph, &recipe.blueprint) || !farmed(graph, &result) {
            continue;
        }
        let ways = graph
            .into(&result)
            .iter()
            .filter(|e| matches!(e.rel, Rel::Drops(_)))
            .count();
        out.push(
            Finding::new(
                Layer::Coverage,
                "dead-recipe",
                &recipe.blueprint,
                format!("nothing hands over this blueprint, and {result} drops in {ways} places"),
            )
            .about([result.clone()]),
        );
    }
    out
}
