use std::collections::BTreeMap;

use funnel::DropClaim;
use graph::{Graph, Node, Rel};

use crate::names::Index;
use crate::wiki;

/// What another source's account of the drop tables came to.
pub struct Drops {
    pub claims: Vec<DropClaim>,
    /// Places we could tie to one of the wiki's tables.
    pub places: usize,
    /// Rows naming something no catalog item answers to.
    pub unresolved: usize,
    /// Relic rows, left out: which relics a node drops is a live rotation, so a witness a day
    /// behind disagrees about every one of them.
    pub relics: usize,
}

/// Read the wiki's mission tables as claims about our own drop edges. The join is structural:
/// a place points at a star-chart node, the node names its reward tables, and the wiki keys
/// its tables by those names. Which of a node's three tables a place means is written in the
/// place's own heading — the caches table and the extra table are printed separately.
pub fn drops(
    graph: &Graph,
    chart: &[wiki::Node],
    tables: &BTreeMap<String, Vec<wiki::Row>>,
    index: &Index,
) -> Drops {
    let by_key: BTreeMap<&str, &wiki::Node> = chart.iter().map(|w| (w.key.as_str(), w)).collect();
    let mut out = Drops {
        claims: Vec::new(),
        places: 0,
        unresolved: 0,
        relics: 0,
    };

    for node in graph.nodes() {
        let Node::Place(place) = node else { continue };
        let id = node.id();
        let Some(region) = region_of(graph, &id) else {
            continue;
        };
        let Some(wiki_node) = by_key.get(region.as_str()) else {
            continue;
        };
        // The Conclave playlists are printed under a planet and the wiki files them as nodes,
        // but their PvP tables are a different game and not what this catalog is about.
        if place.name.contains("(Conclave)") {
            continue;
        }
        let Some(alias) = alias_for(&place.name, wiki_node) else {
            continue;
        };
        let Some(rows) = tables.get(alias) else {
            continue;
        };

        out.places += 1;
        for row in rows {
            if row.kind == "Relic" {
                out.relics += 1;
                continue;
            }
            match resolve(index, row) {
                Some(item) => out.claims.push(DropClaim {
                    place: id.clone(),
                    item: item.to_string(),
                    chance: row.chance / 100.0,
                    rotation: row.rotation.clone(),
                }),
                None => out.unresolved += 1,
            }
        }
    }
    out
}

/// The star-chart node a place sits on.
fn region_of(graph: &Graph, place: &str) -> Option<String> {
    graph
        .from(place)
        .into_iter()
        .find(|e| e.rel == Rel::At)
        .map(|e| e.to.trim_start_matches("region:").to_string())
}

/// Which of the node's tables a printed place name refers to.
fn alias_for<'a>(printed: &str, node: &'a wiki::Node) -> Option<&'a str> {
    if printed.ends_with("Extra") {
        return node.extra_alias.as_deref();
    }
    if printed.contains("(Caches)") {
        return node.cache_alias.as_deref();
    }
    node.alias.as_deref()
}

/// A wiki row resolved to a catalog path. The wiki names a relic without the word, and its
/// own kind label says when to put it back.
fn resolve<'a>(index: &'a Index, row: &wiki::Row) -> Option<&'a str> {
    if crate::names::is_amount(&row.name) {
        return None;
    }
    if row.kind == "Relic" {
        if let Some(path) = index.get(&format!("{} Relic", row.name)) {
            return Some(path);
        }
    }
    index.get(&row.name)
}
