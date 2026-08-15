use graph::{Graph, Node, Rel};

use crate::finding::{Finding, Layer};

/// Structural rules the catalog must never break.
pub fn check(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    let flag = |out: &mut Vec<Finding>, rule: &str, entity: &str, detail: String| {
        out.push(Finding::new(Layer::Invariant, rule, entity, detail));
    };

    // A recipe that transitively needs its own result is a foundry step no player could
    // finish, and it would make any cost walk endless.
    for item in graph::cycles(graph) {
        flag(
            &mut out,
            "recipe-cycle",
            &item,
            "its recipe needs itself, directly or through another".to_string(),
        );
    }

    for item in graph.items() {
        if !item.unique_name.starts_with('/') {
            flag(
                &mut out,
                "item-path",
                &item.unique_name,
                "not a '/'-rooted DE path".to_string(),
            );
        }
        // A derived tradable needs a slug to be priceable; a person who set it by hand has
        // overruled that, so trust them.
        if let Some(t) = &item.tradable {
            let by_hand = t.sources.iter().any(|s| s.as_str() == "curated");
            if t.value && item.slug.is_none() && !by_hand {
                flag(
                    &mut out,
                    "tradable-needs-slug",
                    &item.unique_name,
                    "tradable without a market slug".to_string(),
                );
            }
        }
        if item.names.en.value.trim().is_empty() {
            flag(
                &mut out,
                "name-empty",
                &item.unique_name,
                "empty English name".to_string(),
            );
        }
    }

    for edge in graph.edges() {
        if !graph.has(&edge.from) || !graph.has(&edge.to) {
            out.push(
                Finding::new(
                    Layer::Invariant,
                    "dangling-edge",
                    &edge.from,
                    format!("-{}-> {}", edge.rel.as_str(), edge.to),
                )
                .about([&edge.to]),
            );
        }
        // The game trades the blueprint, never what it builds — unless a person overruled
        // that by hand for a part the market really does trade (necramech and pet parts).
        if let Rel::Produces = edge.rel {
            let consumed = matches!(graph.get(&edge.from), Some(Node::Recipe(r)) if r.consumed);
            if let (true, Some(Node::Item(i))) = (consumed, graph.get(&edge.to)) {
                if let Some(t) = &i.tradable {
                    let by_hand = t.sources.iter().any(|s| s.as_str() == "curated");
                    if t.value && !by_hand {
                        out.push(
                            Finding::new(
                                Layer::Invariant,
                                "built-is-tradable",
                                &edge.to,
                                format!("assembled by {} yet marked tradable", edge.from),
                            )
                            .about([&edge.from]),
                        );
                    }
                }
            }
        }
    }

    out
}
