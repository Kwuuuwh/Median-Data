use consensus::{Claim, Resolved, Source, resolve};
use graph::{Edge, Graph, Imprint, Names, Node, Rel, imprint_id};
use sources::wfm::WfmItem;

/// The market tag that marks a pet-breeding token.
const TAG: &str = "imprint";

/// Imprint nodes, each linked to the animal it breeds. An imprint is tradable and DE has no
/// entity for it, so its name and slug come from the market; the animal it points at stays
/// the clean DE entity, untouched by the market listing.
pub fn link(graph: &mut Graph, wfm: &[WfmItem]) -> usize {
    let mut linked = 0;
    for w in wfm.iter().filter(|w| w.has_tag(TAG)) {
        let Some(animal) = w.game_ref.as_deref() else {
            continue;
        };
        if !graph.has(animal) {
            continue;
        }
        let Some(en) = w.en_name.clone() else {
            continue;
        };
        let id = imprint_id(&w.slug);
        graph.insert(Node::Imprint(Imprint {
            slug: w.slug.clone(),
            names: Names {
                en: claim(en),
                ru: w.ru_name.clone().map(claim),
            },
            animal: animal.to_string(),
        }));
        graph.link(Edge {
            from: id,
            to: animal.to_string(),
            rel: Rel::Yields,
        });
        linked += 1;
    }
    linked
}

fn claim(value: String) -> Resolved<String> {
    resolve(
        &[Claim {
            source: Source::Wfm,
            value,
        }],
        &[Source::Wfm],
    )
    .expect("one claim resolves")
}
