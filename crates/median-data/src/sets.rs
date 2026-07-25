use consensus::{Claim, Resolved, Source, resolve};
use graph::{Edge, Graph, Names, Node, Rel, Set, set_id};

use crate::bridge::Bridge;

/// Set nodes, with edges to their member parts and to the item they assemble into.
pub fn link(graph: &mut Graph, bridge: &Bridge<'_>) {
    for info in bridge.sets() {
        let Some(en) = info.item.en_name.clone() else {
            continue;
        };
        let id = set_id(&info.item.slug);
        graph.insert(Node::Set(Set {
            slug: info.item.slug.clone(),
            names: Names {
                en: claim(en),
                ru: info.item.ru_name.clone().map(claim),
            },
            ducats: info.item.ducats,
        }));

        for member in &info.members {
            let Some(game_ref) = member.game_ref.as_deref() else {
                continue;
            };
            if graph.has(game_ref) {
                graph.link(Edge {
                    from: id.clone(),
                    to: game_ref.to_string(),
                    rel: Rel::Member,
                });
            }
        }

        if let Some(built) = info.item.game_ref.as_deref() {
            if graph.has(built) {
                graph.link(Edge {
                    from: id.clone(),
                    to: built.to_string(),
                    rel: Rel::Represents,
                });
            }
        }
    }
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
