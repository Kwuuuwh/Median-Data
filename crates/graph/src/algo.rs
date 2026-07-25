use std::collections::{BTreeMap, BTreeSet};

use crate::edge::Rel;
use crate::graph::Graph;

/// The recipe that produces an item, if one does.
pub fn producer<'a>(graph: &'a Graph, item: &str) -> Option<&'a str> {
    graph
        .into(item)
        .into_iter()
        .find(|e| e.rel == Rel::Produces)
        .map(|e| e.from.as_str())
}

/// What a recipe builds.
pub fn produced<'a>(graph: &'a Graph, recipe: &str) -> Option<&'a str> {
    graph
        .from(recipe)
        .into_iter()
        .find(|e| e.rel == Rel::Produces)
        .map(|e| e.to.as_str())
}

/// What a recipe consumes, as ingredient and count.
pub fn ingredients<'a>(graph: &'a Graph, recipe: &str) -> Vec<(&'a str, i64)> {
    graph
        .from(recipe)
        .into_iter()
        .filter_map(|e| match e.rel {
            Rel::Requires { count } => Some((e.to.as_str(), count)),
            _ => None,
        })
        .collect()
}

/// Whether something is handed over as it is — dropped, awarded, sold, or unlocked by clan
/// research — with no foundry step of its own. This is where a cost walk stops: a resource you
/// farm is a base material even when a vestigial recipe for it exists, and DE ships several of
/// those.
pub fn handed_over(graph: &Graph, item: &str) -> bool {
    graph.into(item).iter().any(|e| {
        matches!(
            e.rel,
            Rel::Rewards { .. } | Rel::Drops(_) | Rel::Sells(_) | Rel::Researched(_)
        )
    })
}

/// Whether anything in the graph leads to an item at all: a recipe, a relic, a drop, a set,
/// a vendor, or refining the step below it.
pub fn obtainable(graph: &Graph, item: &str) -> bool {
    graph.into(item).iter().any(|e| {
        matches!(
            e.rel,
            Rel::Produces
                | Rel::Rewards { .. }
                | Rel::Drops(_)
                | Rel::Member
                | Rel::Sells(_)
                | Rel::Refines
                | Rel::Researched(_)
        )
    })
}

/// Everything an item costs in base materials, with quantities. The walk stops at anything
/// handed over directly, and at anything no recipe makes.
pub fn rollup(graph: &Graph, item: &str) -> BTreeMap<String, i64> {
    let mut memo = BTreeMap::new();
    let mut path = BTreeSet::new();
    walk(graph, item, &mut memo, &mut path)
}

/// The same walk for many items, sharing one memo: a warframe and its parts overlap heavily.
pub fn rollup_all<'a>(
    graph: &'a Graph,
    items: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<&'a str, BTreeMap<String, i64>> {
    let mut memo = BTreeMap::new();
    let mut out = BTreeMap::new();
    for item in items {
        let mut path = BTreeSet::new();
        let cost = walk(graph, item, &mut memo, &mut path);
        if cost.keys().any(|leaf| leaf != item) {
            out.insert(item, cost);
        }
    }
    out
}

fn walk(
    graph: &Graph,
    item: &str,
    memo: &mut BTreeMap<String, BTreeMap<String, i64>>,
    path: &mut BTreeSet<String>,
) -> BTreeMap<String, i64> {
    if let Some(known) = memo.get(item) {
        return known.clone();
    }
    // A recipe that needs its own result cannot be walked; treat the second visit as a leaf so
    // the walk ends. `cycles` reports the cycle itself.
    if !path.insert(item.to_string()) {
        return BTreeMap::from([(item.to_string(), 1)]);
    }

    let mut total: BTreeMap<String, i64> = BTreeMap::new();
    match producer(graph, item) {
        None => {
            total.insert(item.to_string(), 1);
        }
        Some(recipe) => {
            for (ingredient, count) in ingredients(graph, recipe) {
                if handed_over(graph, ingredient) {
                    *total.entry(ingredient.to_string()).or_default() += count;
                    continue;
                }
                for (leaf, qty) in walk(graph, ingredient, memo, path) {
                    *total.entry(leaf).or_default() += qty * count;
                }
            }
            // A recipe with no ingredients we hold still costs the thing itself.
            if total.is_empty() {
                total.insert(item.to_string(), 1);
            }
        }
    }

    path.remove(item);
    memo.insert(item.to_string(), total.clone());
    total
}

/// Items whose recipe transitively needs the item itself. This should be empty; a recipe
/// cycle is a foundry no player could ever finish.
pub fn cycles(graph: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    for edge in graph.edges() {
        if edge.rel != Rel::Produces || !done.insert(edge.to.as_str()) {
            continue;
        }
        let mut path = BTreeSet::new();
        if reaches(graph, &edge.to, &edge.to, &mut path) {
            out.push(edge.to.clone());
        }
    }
    out
}

/// Whether `target` is needed, directly or not, to build `item`.
fn reaches(graph: &Graph, item: &str, target: &str, path: &mut BTreeSet<String>) -> bool {
    let Some(recipe) = producer(graph, item) else {
        return false;
    };
    if !path.insert(item.to_string()) {
        return false;
    }
    for (ingredient, _) in ingredients(graph, recipe) {
        if ingredient == target || reaches(graph, ingredient, target, path) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::Edge;
    use crate::node::{Node, Recipe, recipe_id};

    fn recipe(graph: &mut Graph, blueprint: &str, result: &str, parts: &[(&str, i64)]) {
        graph.insert(Node::Recipe(Recipe {
            blueprint: blueprint.to_string(),
            build_price: None,
            build_time: None,
            consumed: true,
            rush_price: None,
        }));
        let id = recipe_id(blueprint);
        graph.link(Edge {
            from: id.clone(),
            to: result.to_string(),
            rel: Rel::Produces,
        });
        for (part, count) in parts {
            graph.link(Edge {
                from: id.clone(),
                to: part.to_string(),
                rel: Rel::Requires { count: *count },
            });
        }
    }

    #[test]
    fn a_cost_walks_down_to_what_nothing_makes() {
        let mut g = Graph::new();
        recipe(
            &mut g,
            "/bp/frame",
            "/frame",
            &[("/chassis", 1), ("/cell", 3)],
        );
        recipe(
            &mut g,
            "/bp/chassis",
            "/chassis",
            &[("/cell", 2), ("/plate", 5)],
        );
        let cost = rollup(&g, "/frame");
        // 3 cells directly plus 2 inside the chassis
        assert_eq!(cost.get("/cell"), Some(&5));
        assert_eq!(cost.get("/plate"), Some(&5));
        assert_eq!(cost.get("/chassis"), None);
    }

    #[test]
    fn the_walk_stops_at_anything_handed_over() {
        use crate::node::{Place, PlaceKind};
        let mut g = Graph::new();
        recipe(&mut g, "/bp/frame", "/frame", &[("/cell", 2)]);
        // a vestigial recipe for the resource itself, of the kind DE still ships
        recipe(&mut g, "/bp/cell", "/cell", &[("/spores", 300)]);
        g.insert(Node::Place(Place {
            name: "Ceres/Exta".into(),
            name_ru: None,
            kind: PlaceKind::Node,
            bounty: None,
        }));
        g.link(Edge {
            from: crate::node::place_id("Ceres/Exta"),
            to: "/cell".into(),
            rel: Rel::Drops(crate::edge::DropInfo {
                rarity: "COMMON".into(),
                chance: 0.1,
                rotation: None,
                stage: None,
                table_chance: None,
            }),
        });
        let cost = rollup(&g, "/frame");
        assert_eq!(cost.get("/cell"), Some(&2));
        assert_eq!(
            cost.get("/spores"),
            None,
            "a farmed resource is a base material"
        );
    }

    #[test]
    fn a_count_multiplies_the_whole_branch() {
        let mut g = Graph::new();
        recipe(&mut g, "/bp/a", "/a", &[("/b", 2)]);
        recipe(&mut g, "/bp/b", "/b", &[("/leaf", 3)]);
        assert_eq!(rollup(&g, "/a").get("/leaf"), Some(&6));
    }

    #[test]
    fn something_nothing_produces_costs_only_itself() {
        let g = Graph::new();
        assert_eq!(rollup(&g, "/cell"), BTreeMap::from([("/cell".into(), 1)]));
    }

    #[test]
    fn a_cycle_is_reported_and_does_not_hang_the_walk() {
        let mut g = Graph::new();
        recipe(&mut g, "/bp/a", "/a", &[("/b", 1)]);
        recipe(&mut g, "/bp/b", "/b", &[("/a", 1)]);
        assert_eq!(cycles(&g).len(), 2);
        let cost = rollup(&g, "/a");
        assert!(!cost.is_empty());
    }

    #[test]
    fn a_straight_chain_is_not_a_cycle() {
        let mut g = Graph::new();
        recipe(&mut g, "/bp/a", "/a", &[("/b", 1)]);
        recipe(&mut g, "/bp/b", "/b", &[("/leaf", 1)]);
        assert!(cycles(&g).is_empty());
    }
}
