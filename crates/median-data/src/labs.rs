use graph::{Edge, Graph, Lab, Node, Rel, Research, lab_id};

use crate::names::{self, Index};
use crate::orphans::{self, Missing};
use crate::wiki::Dojo;

/// What tying dojo research to the catalog produced.
pub struct Linked {
    pub labs: usize,
    pub research: usize,
    /// Names no catalog item answers to: dojo decorations and bundles.
    pub unresolved: Missing,
}

/// Add the dojo labs and link each research to the blueprint it unlocks. Researching does not
/// hand over the thing itself — it lets the clan buy its blueprint, and building it then costs
/// what the foundry recipe says. So the edge lands on the blueprint, which is exactly the item
/// that otherwise looks obtainable from nowhere.
pub fn link(
    graph: &mut Graph,
    dojo: &Dojo,
    index: &Index,
    terms: &crate::curation::Terms,
) -> Linked {
    let mut out = Linked {
        labs: 0,
        research: 0,
        unresolved: Missing::new(),
    };

    for lab in &dojo.labs {
        if graph.insert(Node::Lab(Lab {
            key: lab.key.clone(),
            name: lab.name.clone(),
            name_ru: terms.get("lab", &lab.key).map(str::to_string),
            faction: lab.faction.clone(),
        })) {
            out.labs += 1;
        }
    }

    let mut edges = Vec::new();
    for it in &dojo.research {
        let (_, printed) = names::quantity(&it.name);
        let Some(item) = index.get(printed) else {
            orphans::note(&mut out.unresolved, printed, || it.lab.clone());
            continue;
        };
        let target = graph::producer(graph, item)
            .and_then(|recipe| blueprint_of(graph, recipe))
            .unwrap_or(item)
            .to_string();
        edges.push(Edge {
            from: lab_id(&it.lab),
            to: target,
            rel: Rel::Researched(Research {
                credits: it.credits,
                time: it.time,
                affinity: it.affinity,
                prereq: it.prereq.clone(),
                resources: it.resources.clone(),
            }),
        });
    }

    out.research = edges.len();
    for edge in edges {
        graph.link(edge);
    }
    out
}

/// The blueprint a recipe is keyed by.
fn blueprint_of<'a>(graph: &'a Graph, recipe: &str) -> Option<&'a str> {
    match graph.get(recipe)? {
        Node::Recipe(r) => Some(&r.blueprint),
        _ => None,
    }
}
