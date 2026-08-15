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
            vaulted: None,
        }));

        // Read through the bridge rather than off the listing's own reference: the market
        // points a `…_blueprint` listing at the component that blueprint builds, and the set
        // is made of the parts it trades.
        for member in &info.members {
            let Some((path, _)) = bridge.matched().get(&member.slug) else {
                continue;
            };
            if graph.has(path) {
                graph.link(Edge {
                    from: id.clone(),
                    to: path.clone(),
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
