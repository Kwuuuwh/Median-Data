use std::collections::{BTreeMap, BTreeSet};

use consensus::{Claim, Conflict, Source, claims, resolve};
use graph::{Edge, Extra, Graph, Node, Rel};

use crate::extract::DeReward;
use crate::names::Index;
use crate::wiki::Slot;

/// Who outranks whom on what a relic contains. DE ships the game; the wiki is written by the
/// people who open the relics.
const SOURCES: [Source; 2] = [Source::De, Source::Wiki];

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
                count: r.count,
            },
        });
    }
    unresolved
}

/// What checking DE's account of relic contents against the wiki's came to.
#[derive(Default)]
pub struct Witnessed {
    /// Relics both sources describe.
    pub compared: usize,
    /// Relics only DE describes, so nothing can contradict it.
    pub alone: usize,
    /// Reward names the wiki prints that no catalog item answers to.
    pub unresolved: BTreeSet<String>,
}

/// Check what DE says a relic awards against what the wiki says. DE keeps the value — the
/// wiki never outranks it — but the disagreement stops being invisible, which is the one
/// thing a single source can never do for itself.
pub fn witnessed(
    graph: &Graph,
    index: &Index,
    wiki: &[Slot],
    conflicts: &mut Vec<Conflict>,
) -> Witnessed {
    let mut out = Witnessed::default();

    let mut ours: BTreeMap<&str, Vec<Held<'_>>> = BTreeMap::new();
    for edge in graph.edges() {
        if let Rel::Rewards { rarity, count } = &edge.rel {
            ours.entry(&edge.from)
                .or_default()
                .push((&edge.to, rarity.to_uppercase(), *count));
        }
    }

    let mut theirs: BTreeMap<&str, Vec<Held<'_>>> = BTreeMap::new();
    for slot in wiki {
        let Some(reward) = index.get(&slot.reward) else {
            out.unresolved.insert(slot.reward.clone());
            continue;
        };
        // The wiki names a relic without the word the catalog keeps on it: `Lith G12`
        // against `Lith G12 Relic`.
        let named = format!("{} Relic", slot.relic);
        for refinement in REFINEMENTS {
            for relic in index.relics(&named, refinement) {
                if let Some((path, _)) = ours.get_key_value(relic.as_str()) {
                    theirs.entry(path).or_default().push((
                        reward,
                        slot.rarity.to_uppercase(),
                        slot.count,
                    ));
                }
            }
        }
    }

    for (relic, mut mine) in ours {
        let Some(mut yours) = theirs.remove(relic) else {
            out.alone += 1;
            continue;
        };
        out.compared += 1;
        mine.sort_unstable();
        yours.sort_unstable();
        let (de, wiki) = (printed(graph, &mine), printed(graph, &yours));
        let picked = [
            Claim {
                source: Source::De,
                value: de.clone(),
            },
            Claim {
                source: Source::Wiki,
                value: wiki.clone(),
            },
        ];
        let Some(settled) = resolve(&picked, &SOURCES) else {
            continue;
        };
        if settled.status == consensus::Status::Conflict {
            conflicts.push(Conflict::new(
                relic,
                "rewards",
                claims(&[(Source::De, Some(de)), (Source::Wiki, Some(wiki))]),
                settled.value,
            ));
        }
    }
    out
}

/// One slot of a relic: what it awards, how rare it is and how many it hands over.
type Held<'a> = (&'a str, String, i64);

/// One relic's slots as a line a person can compare by eye.
fn printed(graph: &Graph, slots: &[Held<'_>]) -> String {
    slots
        .iter()
        .map(|(path, rarity, count)| {
            let name = match graph.get(path) {
                Some(Node::Item(item)) => item.names.en.value.as_str(),
                _ => path,
            };
            let many = if *count > 1 {
                format!(" x{count}")
            } else {
                String::new()
            };
            format!("{name}{many} ({})", rarity.to_lowercase())
        })
        .collect::<Vec<_>>()
        .join(", ")
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
