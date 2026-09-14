use std::collections::{BTreeMap, BTreeSet};

use consensus::{Claim, Resolved, Source, resolve};
use graph::{Extra, Graph, Node, Rel};
use sources::wfm::WfmItem;

use crate::wiki::Vaulting;

/// Which source is believed about the vault. The wiki states the rule the game plays by — a
/// relic is vaulted when it drops in no mission — while the market calls a relic available
/// as long as its Prime Vault pack is on sale, which is a different question.
const SOURCES: [Source; 3] = [Source::Curated, Source::Wiki, Source::Wfm];

/// What the pass settled.
#[derive(Default)]
pub struct Vaulted {
    /// Relics a source spoke about.
    pub relics: usize,
    /// Items whose status was derived from the relics that award them.
    pub derived: usize,
    /// Relic names the wiki lists that the catalog does not hold.
    pub unmatched: Vec<String>,
    /// Derived statuses withdrawn because the thing is handed over some other way.
    pub contradicted: usize,
}

/// Mark what is in the vault. A relic is stated by a source; a prime part is vaulted when
/// every relic awarding it is; an assembled item when every part of it is. Nothing is
/// guessed: an item no relic leads to keeps no status at all.
pub fn mark(
    graph: &mut Graph,
    wiki: &[Vaulting],
    wfm: &[WfmItem],
    matched: &BTreeMap<String, String>,
    curated: &BTreeMap<(&str, &str), &str>,
    conflicts: &mut Vec<consensus::Conflict>,
) -> Vaulted {
    let mut out = Vaulted::default();
    let by_name = relic_paths(graph);

    let mut claims: BTreeMap<String, Vec<Claim<bool>>> = BTreeMap::new();
    let mut since: BTreeMap<String, String> = BTreeMap::new();
    for row in wiki {
        let Some(bases) = by_name.get(row.relic.as_str()) else {
            out.unmatched.push(row.relic.clone());
            continue;
        };
        for base in bases {
            claims.entry((*base).to_string()).or_default().push(Claim {
                source: Source::Wiki,
                value: row.since.is_some(),
            });
            if let Some(version) = &row.since {
                since.insert((*base).to_string(), version.clone());
            }
        }
    }
    // The listing is read through the path the bridge tied it to: the market's own reference
    // names the relic without its refinement, which is not an entity DE exports.
    for item in wfm {
        let (Some(vaulted), Some(path)) = (item.vaulted, matched.get(&item.slug)) else {
            continue;
        };
        if let Some(base) = base_of(graph, path) {
            claims.entry(base).or_default().push(Claim {
                source: Source::Wfm,
                value: vaulted,
            });
        }
    }

    let mut settled: BTreeMap<String, Resolved<bool>> = BTreeMap::new();
    for (base, mut list) in claims {
        if let Some(hand) = curated.get(&(base.as_str(), "vaulted")).and_then(flag) {
            list.insert(
                0,
                Claim {
                    source: Source::Curated,
                    value: hand,
                },
            );
        }
        if let Some(value) = resolve(&list, &SOURCES) {
            if value.status == consensus::Status::Conflict {
                conflicts.push(consensus::Conflict::new(
                    base.clone(),
                    "vaulted",
                    list.iter()
                        .map(|c| (c.source, c.value.to_string()))
                        .collect(),
                    value.value.to_string(),
                ));
            }
            settled.insert(base, value);
        }
    }
    out.relics = settled.len();

    // Every refinement of a relic shares its status: they are one relic to a player.
    let mut status: BTreeMap<String, Resolved<bool>> = BTreeMap::new();
    let mut stamps: BTreeMap<String, String> = BTreeMap::new();
    for item in graph.items() {
        if let Extra::Relic(r) = &item.extra
            && let Some(value) = settled.get(&r.base)
        {
            status.insert(item.unique_name.clone(), value.clone());
            if let Some(version) = since.get(&r.base) {
                stamps.insert(item.unique_name.clone(), version.clone());
            }
        }
    }

    let rewarded = derive(
        graph,
        &status,
        Rel::Rewards {
            rarity: String::new(),
            count: 0,
            chance: None,
        },
    );
    out.derived = rewarded.len();
    status.extend(rewarded);
    out.contradicted = clear_obtainable(graph, &mut status);

    let built = from_blueprints(graph, &status);
    out.derived += built.len();
    status.extend(built);
    out.contradicted += clear_obtainable(graph, &mut status);

    let assembled = derive(graph, &status, Rel::Requires { count: 0 });
    out.derived += assembled.len();
    status.extend(assembled);
    out.contradicted += clear_obtainable(graph, &mut status);

    let traded = from_members(graph, &status);
    out.derived += traded.len();
    status.extend(traded);

    for node in graph.nodes_mut() {
        match node {
            Node::Item(item) => {
                if let Some(value) = status.get(&item.unique_name) {
                    item.vaulted = Some(value.clone());
                }
                if let (Extra::Relic(r), Some(version)) =
                    (&mut item.extra, stamps.get(&item.unique_name))
                {
                    r.vaulted_in = Some(version.clone());
                }
            }
            Node::Set(set) => {
                if let Some(value) = status.get(&graph::set_id(&set.slug)) {
                    set.vaulted = Some(value.clone());
                }
            }
            _ => {}
        }
    }
    out
}

/// Withdraw a derived "vaulted" from anything the game still hands over another way: a relic
/// reward that also drops, is sold, or comes out of clan research is not in the vault, whatever
/// the relics say. What a source stated is left alone — a source outranks our own reasoning.
fn clear_obtainable(graph: &Graph, status: &mut BTreeMap<String, Resolved<bool>>) -> usize {
    let mut cleared = 0;
    for (id, value) in status.iter_mut() {
        if !value.value || value.winner != Source::Rule {
            continue;
        }
        let elsewhere = graph.into(id).iter().any(|e| {
            matches!(
                e.rel,
                Rel::Drops(_) | Rel::Sells(_) | Rel::Researched(_) | Rel::Refines
            )
        });
        if elsewhere {
            value.value = false;
            cleared += 1;
        }
    }
    cleared
}

/// A trade set is in the vault when every part of it is.
fn from_members(
    graph: &Graph,
    known: &BTreeMap<String, Resolved<bool>>,
) -> BTreeMap<String, Resolved<bool>> {
    let mut seen: BTreeMap<String, (bool, Vec<Source>)> = BTreeMap::new();
    for edge in graph.edges() {
        if edge.rel != Rel::Member {
            continue;
        }
        let Some(value) = known.get(&edge.to) else {
            continue;
        };
        let slot = seen.entry(edge.from.clone()).or_insert((true, Vec::new()));
        slot.0 &= value.value;
        for source in &value.sources {
            if !slot.1.contains(source) {
                slot.1.push(*source);
            }
        }
    }
    seen.into_iter()
        .map(|(id, (vaulted, sources))| (id, rule(vaulted, &sources)))
        .collect()
}

/// Carry the status one step along a relation: a thing is vaulted when everything the
/// relation leads to it from is, and available as soon as one of them is. Only what the
/// sources already settled counts, so a part nothing leads to keeps no status.
fn derive(
    graph: &Graph,
    known: &BTreeMap<String, Resolved<bool>>,
    rel: Rel,
) -> BTreeMap<String, Resolved<bool>> {
    let mut seen: BTreeMap<String, (bool, Vec<Source>)> = BTreeMap::new();
    for edge in graph.edges() {
        if std::mem::discriminant(&edge.rel) != std::mem::discriminant(&rel) {
            continue;
        }
        // `Requires` runs recipe -> ingredient, so the assembled item is read off the recipe.
        let (from, to) = match rel {
            Rel::Requires { .. } => match graph::produced(graph, &edge.from) {
                Some(built) => (edge.to.as_str(), built),
                None => continue,
            },
            _ => (edge.from.as_str(), edge.to.as_str()),
        };
        let Some(value) = known.get(from) else {
            continue;
        };
        let slot = seen.entry(to.to_string()).or_insert((true, Vec::new()));
        slot.0 &= value.value;
        for source in &value.sources {
            if !slot.1.contains(source) {
                slot.1.push(*source);
            }
        }
    }

    seen.into_iter()
        .filter(|(id, _)| !known.contains_key(id))
        .map(|(id, (vaulted, sources))| (id, rule(vaulted, &sources)))
        .collect()
}

/// A status the build worked out itself, carrying the sources it rests on.
fn rule(vaulted: bool, sources: &[Source]) -> Resolved<bool> {
    let mut all: Vec<Source> = sources.to_vec();
    if !all.contains(&Source::Rule) {
        all.push(Source::Rule);
    }
    Resolved {
        value: vaulted,
        status: consensus::Status::Single,
        winner: Source::Rule,
        sources: all,
    }
}

/// What a recipe builds inherits the status of the blueprint that starts it: a blueprint in
/// the vault is the only way to what it builds.
fn from_blueprints(
    graph: &Graph,
    known: &BTreeMap<String, Resolved<bool>>,
) -> BTreeMap<String, Resolved<bool>> {
    let mut out = BTreeMap::new();
    for edge in graph.edges() {
        if edge.rel != Rel::Produces || known.contains_key(&edge.to) {
            continue;
        }
        let Some(value) = edge
            .from
            .strip_prefix("recipe:")
            .and_then(|bp| known.get(bp))
        else {
            continue;
        };
        out.insert(edge.to.clone(), rule(value.value, &value.sources));
    }
    out
}

/// Printed relic name (`Axi A1`) to every logical relic answering to it. DE keeps a separate
/// entity per Prime Vault re-release, and they share the printed name.
fn relic_paths(graph: &Graph) -> BTreeMap<&str, Vec<&str>> {
    let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for item in graph.items() {
        if let Extra::Relic(r) = &item.extra
            && r.refinement == "intact"
        {
            let bases = out
                .entry(item.names.en.value.trim_end_matches(" Relic"))
                .or_default();
            if !bases.contains(&r.base.as_str()) {
                bases.push(r.base.as_str());
            }
        }
    }
    out
}

/// The logical relic a path belongs to, whatever refinement the path names.
fn base_of(graph: &Graph, path: &str) -> Option<String> {
    match graph.get(path) {
        Some(Node::Item(item)) => match &item.extra {
            Extra::Relic(r) => Some(r.base.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn flag(value: &&str) -> Option<bool> {
    match *value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Relic paths whose status no source stated, for the report.
pub fn unstated(graph: &Graph) -> BTreeSet<String> {
    graph
        .items()
        .filter(|i| matches!(&i.extra, Extra::Relic(r) if r.refinement == "intact"))
        .filter(|i| i.vaulted.is_none())
        .map(|i| i.unique_name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus::{Claim, resolve};
    use graph::{DropInfo, Edge, Item, Names, Place, PlaceKind, RelicInfo, place_id};

    fn value<T: Clone + PartialEq>(v: T) -> Resolved<T> {
        resolve(
            &[Claim {
                source: Source::De,
                value: v,
            }],
            &[Source::De],
        )
        .expect("one claim resolves")
    }

    fn item(path: &str, name: &str, extra: Extra) -> Node {
        Node::Item(Item {
            unique_name: path.into(),
            names: Names {
                en: value(name.to_string()),
                ru: None,
            },
            category: value("Test".to_string()),
            kind: value(graph::Kind::unknown()),
            slug: None,
            tradable: None,
            vaulted: None,
            prime: value(true),
            ducats: None,
            mastery: None,
            mastery_req: None,
            max_level_cap: None,
            extra,
        })
    }

    fn relic(path: &str, name: &str) -> Node {
        item(
            path,
            name,
            Extra::Relic(RelicInfo {
                base: path.into(),
                refinement: "intact".into(),
                vaulted_in: None,
            }),
        )
    }

    fn wiki(name: &str, since: Option<&str>) -> Vaulting {
        Vaulting {
            relic: name.into(),
            since: since.map(str::to_string),
        }
    }

    fn awards(graph: &mut Graph, relic: &str, item: &str) {
        graph.link(Edge {
            from: relic.into(),
            to: item.into(),
            rel: Rel::Rewards {
                rarity: "Common".into(),
                count: 1,
                chance: None,
            },
        });
    }

    /// One vaulted relic and one that still drops, each awarding a part of its own.
    fn catalog() -> Graph {
        let mut graph = Graph::new();
        graph.insert(relic("/r/gone", "Axi A1 Relic"));
        graph.insert(relic("/r/live", "Axi A2 Relic"));
        graph.insert(item("/p/gone", "Ash Prime Blueprint", Extra::None));
        graph.insert(item("/p/shared", "Forma Blueprint", Extra::None));
        awards(&mut graph, "/r/gone", "/p/gone");
        awards(&mut graph, "/r/gone", "/p/shared");
        awards(&mut graph, "/r/live", "/p/shared");
        graph
    }

    fn mark_with(graph: &mut Graph, rows: &[Vaulting]) -> Vaulted {
        mark(
            graph,
            rows,
            &[],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &mut Vec::new(),
        )
    }

    fn vaulted_of(graph: &Graph, path: &str) -> Option<bool> {
        match graph.get(path) {
            Some(Node::Item(i)) => i.vaulted.as_ref().map(|v| v.value),
            _ => None,
        }
    }

    #[test]
    fn a_part_is_vaulted_only_when_every_relic_awarding_it_is() {
        let mut graph = catalog();
        mark_with(
            &mut graph,
            &[wiki("Axi A1", Some("21.6")), wiki("Axi A2", None)],
        );
        assert_eq!(vaulted_of(&graph, "/r/gone"), Some(true));
        assert_eq!(vaulted_of(&graph, "/r/live"), Some(false));
        assert_eq!(vaulted_of(&graph, "/p/gone"), Some(true));
        // awarded by a relic that still drops, so it is not in the vault
        assert_eq!(vaulted_of(&graph, "/p/shared"), Some(false));
    }

    #[test]
    fn what_still_drops_is_not_in_the_vault_whatever_the_relics_say() {
        let mut graph = catalog();
        graph.insert(Node::Place(Place {
            name: "Earth/Mariana".into(),
            name_ru: None,
            kind: PlaceKind::Node,
            bounty: None,
            table: None,
        }));
        graph.link(Edge {
            from: place_id("Earth/Mariana"),
            to: "/p/gone".into(),
            rel: Rel::Drops(DropInfo {
                rarity: "Common".into(),
                chance: 0.1,
                rotation: None,
                stage: None,
                table_chance: None,
                levels: None,
                count: None,
            }),
        });
        let out = mark_with(&mut graph, &[wiki("Axi A1", Some("21.6"))]);
        assert_eq!(vaulted_of(&graph, "/p/gone"), Some(false));
        assert_eq!(out.contradicted, 1);
    }

    #[test]
    fn a_relic_the_catalog_does_not_hold_is_reported() {
        let mut graph = catalog();
        let out = mark_with(&mut graph, &[wiki("Lith Z9", Some("40"))]);
        assert_eq!(out.unmatched, vec!["Lith Z9".to_string()]);
    }
}
