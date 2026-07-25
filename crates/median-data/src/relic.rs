use std::collections::{BTreeMap, BTreeSet};

use graph::{Edge, Extra, Graph, Rel};

use crate::extract::DeReward;

/// The order a relic is refined in. Only the intact one drops.
const REFINEMENTS: [&str; 4] = ["intact", "exceptional", "flawless", "radiant"];

/// Reward edges from each relic to the items it can award. Rewards naming something the
/// graph does not hold are skipped and reported.
pub fn link(graph: &mut Graph, rewards: &[DeReward]) -> BTreeSet<String> {
    let mut unresolved = BTreeSet::new();
    for r in rewards {
        if !graph.has(&r.relic) {
            unresolved.insert(r.relic.clone());
            continue;
        }
        if !graph.has(&r.reward) {
            unresolved.insert(r.reward.clone());
            continue;
        }
        graph.link(Edge {
            from: r.relic.clone(),
            to: r.reward.clone(),
            rel: Rel::Rewards {
                rarity: r.rarity.clone(),
            },
        });
    }
    unresolved
}

/// Chain each relic's four refinements. Without this the three refined ones look obtainable
/// from nowhere: no drop table lists them, because you make them from the intact relic.
pub fn refine(graph: &mut Graph) -> usize {
    let mut by_base: BTreeMap<(String, String), String> = BTreeMap::new();
    for item in graph.items() {
        if let Extra::Relic(r) = &item.extra {
            by_base.insert(
                (r.base.clone(), r.refinement.clone()),
                item.unique_name.clone(),
            );
        }
    }

    let bases: BTreeSet<String> = by_base.keys().map(|(base, _)| base.clone()).collect();
    let mut edges = Vec::new();
    for base in bases {
        for pair in REFINEMENTS.windows(2) {
            let (from, to) = (
                by_base.get(&(base.clone(), pair[0].to_string())),
                by_base.get(&(base.clone(), pair[1].to_string())),
            );
            if let (Some(from), Some(to)) = (from, to) {
                edges.push(Edge {
                    from: from.clone(),
                    to: to.clone(),
                    rel: Rel::Refines,
                });
            }
        }
    }
    let count = edges.len();
    for edge in edges {
        graph.link(edge);
    }
    count
}
