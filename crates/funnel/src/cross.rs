use std::collections::{BTreeMap, BTreeSet};

use graph::{Graph, Node, Rel, Taxonomy};

use crate::finding::{Finding, Layer};

/// A relic reward as printed by an independent source, resolved to catalog paths. The
/// printed rarity label is not comparable across sources; the printed chance is.
pub struct RelicClaim {
    pub relic: String,
    pub reward: String,
    pub chance: f64,
}

/// A drop row as an independent source prints it, resolved to catalog ids.
pub struct DropClaim {
    pub place: String,
    pub item: String,
    pub chance: f64,
    /// Which rotation the row sits in, where the table has them.
    pub rotation: Option<String>,
}

/// Drop chances are printed rounded to two decimals.
const CHANCE_TOLERANCE: f64 = 0.0005;

/// Classes whose items are parts of something bigger rather than things in their own right.
const PART_CLASSES: [&str; 2] = ["component", "blueprint"];

/// Check what the taxonomy calls an item against what the graph shows it doing. Set
/// composition comes from the market, which never read the rule table, so the two are
/// independent accounts of the same thing.
pub fn taxonomy_against_sets(graph: &Graph, taxonomy: &Taxonomy) -> Vec<Finding> {
    let mut out = Vec::new();
    for item in graph.items() {
        let class = taxonomy.class_slug(&item.kind.value);
        let in_a_set = graph
            .into(&item.unique_name)
            .iter()
            .any(|e| matches!(e.rel, Rel::Member));

        if in_a_set && !PART_CLASSES.contains(&class) {
            out.push(Finding::new(
                Layer::Cross,
                "kind-not-a-part",
                &item.unique_name,
                format!("belongs to a set, yet the rules call it {class}"),
            ));
        }
        // Only for what the market trades: a part nobody sells needs no set to belong to.
        let traded = item.tradable.as_ref().is_some_and(|t| t.value);
        if traded && !in_a_set && class == "component" {
            out.push(Finding::new(
                Layer::Cross,
                "part-without-set",
                &item.unique_name,
                "traded as a part, yet belongs to no set".to_string(),
            ));
        }
    }
    out
}

/// Compare the drop rows the catalog took from the official tables against another source's
/// account of the same table. Only places the witness actually covers are judged: it knows
/// mission rewards, not enemies or containers, so silence about a place is not a claim.
///
/// Which relics a node drops is a live rotation — DE moves them in and out of the vault — so
/// a witness that is a day behind disagrees about every one of them. Relics are left to the
/// relic check, which compares them against DE itself.
pub fn drops(graph: &Graph, witness: &[DropClaim]) -> Vec<Finding> {
    let mut theirs: BTreeMap<(&str, &str), &DropClaim> = BTreeMap::new();
    let mut covered: BTreeSet<&str> = BTreeSet::new();
    for claim in witness {
        if is_relic(graph, &claim.item) {
            continue;
        }
        theirs.insert((&claim.place, &claim.item), claim);
        covered.insert(&claim.place);
    }

    let mut ours: BTreeSet<(&str, &str)> = BTreeSet::new();
    for edge in graph.edges() {
        if matches!(edge.rel, Rel::Drops(_))
            && covered.contains(edge.from.as_str())
            && !is_relic(graph, &edge.to)
        {
            ours.insert((&edge.from, &edge.to));
        }
    }

    let mut out = Vec::new();
    for pair in &ours {
        if !theirs.contains_key(pair) {
            out.push(
                Finding::new(
                    Layer::Cross,
                    "drop-unwitnessed",
                    pair.0,
                    format!("{} drops here in the official tables only", pair.1),
                )
                .about([pair.1]),
            );
        }
    }
    for (pair, claim) in &theirs {
        if !ours.contains(pair) {
            let rotation = claim
                .rotation
                .as_deref()
                .map(|r| format!(" (rotation {r})"))
                .unwrap_or_default();
            out.push(
                Finding::new(
                    Layer::Cross,
                    "drop-only-on-the-wiki",
                    pair.0,
                    format!(
                        "the wiki drops {} here at {:.2}%{rotation}, the official tables do not",
                        pair.1,
                        claim.chance * 100.0
                    ),
                )
                .about([pair.1]),
            );
        }
    }
    out
}

/// Compare the relic rewards the catalog took from DE against another source's account of
/// the same thing. Rarity labels are not comparable across sources, so each side is reduced
/// to the chance it implies. A relic can award the same item in two slots, so both sides are
/// summed per pair rather than kept one-to-one — otherwise a slot silently disappears.
pub fn relic_rewards(graph: &Graph, witness: &[RelicClaim]) -> Vec<Finding> {
    let mut ours: BTreeMap<(&str, &str), f64> = BTreeMap::new();
    for edge in graph.edges() {
        let Rel::Rewards { rarity, .. } = &edge.rel else {
            continue;
        };
        let odds = refinement_of(graph, &edge.from).and_then(|r| graph::chance(rarity, r));
        *ours.entry((&edge.from, &edge.to)).or_default() += odds.unwrap_or_default();
    }

    let mut theirs: BTreeMap<(&str, &str), f64> = BTreeMap::new();
    for claim in witness {
        *theirs.entry((&claim.relic, &claim.reward)).or_default() += claim.chance;
    }

    let mut out = Vec::new();
    for (pair, odds) in &ours {
        let Some(theirs) = theirs.get(pair) else {
            out.push(
                Finding::new(
                    Layer::Cross,
                    "relic-reward-unwitnessed",
                    pair.0,
                    format!("awards {} in DE only", pair.1),
                )
                .about([pair.1]),
            );
            continue;
        };
        // Zero means no refinement or no chance for the rarity, not a real disagreement.
        if *odds == 0.0 {
            continue;
        }
        if (odds - theirs).abs() > CHANCE_TOLERANCE {
            out.push(
                Finding::new(
                    Layer::Cross,
                    "relic-chance-differs",
                    pair.0,
                    format!(
                        "{}: DE implies {:.2}%, drop tables print {:.2}%",
                        pair.1,
                        odds * 100.0,
                        theirs * 100.0
                    ),
                )
                .about([pair.1]),
            );
        }
    }
    for pair in theirs.keys() {
        if !ours.contains_key(pair) {
            out.push(
                Finding::new(
                    Layer::Cross,
                    "relic-reward-missing",
                    pair.0,
                    format!("drop tables award {}, DE does not", pair.1),
                )
                .about([pair.1]),
            );
        }
    }
    out
}

fn is_relic(graph: &Graph, item: &str) -> bool {
    refinement_of(graph, item).is_some()
}

pub(crate) fn refinement_of<'a>(graph: &'a Graph, relic: &str) -> Option<&'a str> {
    match graph.get(relic)? {
        Node::Item(item) => match &item.extra {
            graph::Extra::Relic(r) => Some(&r.refinement),
            graph::Extra::None => None,
        },
        _ => None,
    }
}

/// Check the vault status against our own drop tables: what is in the vault is out of the
/// game's tables by definition, so a vaulted item that still drops means one of the two is
/// wrong.
pub fn vault_against_drops(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    for item in graph.items() {
        if !item.vaulted.as_ref().is_some_and(|v| v.value) {
            continue;
        }
        let places: Vec<&str> = graph
            .into(&item.unique_name)
            .iter()
            .filter(|e| matches!(e.rel, Rel::Drops(_)))
            .map(|e| e.from.as_str())
            .collect();
        if places.is_empty() {
            continue;
        }
        out.push(
            Finding::new(
                Layer::Cross,
                "vaulted-yet-drops",
                &item.unique_name,
                format!("called vaulted yet drops in {} place(s)", places.len()),
            )
            .about(places.iter().take(4).copied()),
        );
    }
    out
}

/// Compare each trade set's membership, as the market lists it, against the parts the DE
/// recipe actually needs.
pub fn set_composition(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    for node in graph.nodes() {
        let Node::Set(set) = node else {
            continue;
        };
        let id = node.id();

        let listed: BTreeSet<&str> = graph
            .from(&id)
            .into_iter()
            .filter(|e| matches!(e.rel, Rel::Member))
            .map(|e| e.to.as_str())
            .collect();
        let Some(built) = graph
            .from(&id)
            .into_iter()
            .find(|e| matches!(e.rel, Rel::Represents))
            .map(|e| e.to.clone())
        else {
            continue;
        };
        let Some(required) = parts_of(graph, &built) else {
            continue;
        };

        let missing: Vec<&str> = required.difference(&listed).copied().collect();
        let extra: Vec<&str> = listed.difference(&required).copied().collect();
        if !missing.is_empty() || !extra.is_empty() {
            out.push(Finding::new(
                Layer::Cross,
                "set-composition-differs",
                &set.slug,
                format!(
                    "market lists {} part(s), DE needs {}; only in DE: [{}]; only on market: [{}]",
                    listed.len(),
                    required.len(),
                    missing.join(", "),
                    extra.join(", ")
                ),
            ).about(missing.iter().chain(&extra).copied()));
        }
    }
    out
}

/// The tradable parts a set must contain: the blueprint that assembles the item, plus one
/// per part the assembly consumes.
fn parts_of<'a>(graph: &'a Graph, built: &str) -> Option<BTreeSet<&'a str>> {
    let recipe = graph
        .into(built)
        .into_iter()
        .find(|e| matches!(e.rel, Rel::Produces))?;
    let mut parts = BTreeSet::new();
    parts.insert(strip_recipe(&recipe.from));

    for edge in graph.from(&recipe.from) {
        let Rel::Requires { .. } = edge.rel else {
            continue;
        };
        if let Some(part) = tradable_part(graph, &edge.to) {
            parts.insert(part);
        }
    }
    Some(parts)
}

/// What a set sells for an ingredient. A warframe component is built, so the set carries
/// its blueprint; a weapon part is dropped whole, so the set carries the part itself. Raw
/// resources are neither.
fn tradable_part<'a>(graph: &'a Graph, ingredient: &'a str) -> Option<&'a str> {
    let built_by = graph
        .into(ingredient)
        .into_iter()
        .find(|e| matches!(e.rel, Rel::Produces));
    if let Some(sub) = built_by
        && matches!(graph.get(&sub.from), Some(Node::Recipe(r)) if r.consumed)
    {
        return Some(strip_recipe(&sub.from));
    }
    match graph.get(ingredient) {
        Some(Node::Item(i)) if i.tradable.as_ref().is_some_and(|t| t.value) => Some(ingredient),
        _ => None,
    }
}

fn strip_recipe(id: &str) -> &str {
    id.strip_prefix("recipe:").unwrap_or(id)
}
