use std::collections::BTreeMap;

use graph::{Edge, Graph, Rel};

/// Where the operator's wardrobe lives. Everything the rule looks at sits under this path.
const WARDROBE: &str = "/Lotus/Upgrades/Skins/Operator/";

/// What DE marks the Drifter's copy of a piece with. The Drifter is the grown Operator, so
/// the suffix rides on the body-part path: `SleevesDaxB` is the Operator's, `SleevesAdultDaxB`
/// is the Drifter's.
const ADULT: &str = "Adult";

/// Link each operator cosmetic to the Drifter's copy of it. One purchase hands over both, but
/// DE ships them as two entities under one display name, so without this the Drifter's half
/// comes from nowhere and the pair reads as a duplicate.
///
/// The match is a name shared by exactly two pieces of the wardrobe, exactly one of them
/// marked as the Drifter's. A name carried by two pieces that are not such a pair — the
/// re-issued facial markings, which are both the Operator's — is left alone.
pub fn link(graph: &mut Graph) -> usize {
    let mut worn: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for item in graph.items() {
        if item.unique_name.starts_with(WARDROBE) {
            worn.entry(item.names.en.value.as_str())
                .or_default()
                .push(item.unique_name.as_str());
        }
    }

    let mut edges = Vec::new();
    for paths in worn.values() {
        let [first, second] = paths.as_slice() else {
            continue;
        };
        let pair = match (first.contains(ADULT), second.contains(ADULT)) {
            (false, true) => (first, second),
            (true, false) => (second, first),
            _ => continue,
        };
        edges.push(Edge {
            from: pair.0.to_string(),
            to: pair.1.to_string(),
            rel: Rel::Fits,
        });
    }

    let count = edges.len();
    for edge in edges {
        graph.link(edge);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus::{Claim, Resolved, Source, resolve};
    use graph::{Extra, Item, Names, Node};

    fn value<T: Clone + PartialEq>(v: T) -> Resolved<T> {
        resolve(
            &[Claim {
                source: Source::De,
                value: v,
            }],
            &[Source::De],
        )
        .unwrap()
    }

    fn item(path: &str, name: &str) -> Node {
        Node::Item(Item {
            unique_name: path.into(),
            names: Names {
                en: value(name.to_string()),
                ru: None,
            },
            category: value("Skins".to_string()),
            kind: value(graph::Kind::unknown()),
            slug: None,
            tradable: None,
            prime: value(false),
            ducats: None,
            extra: Extra::None,
        })
    }

    fn wardrobe(pieces: &[(&str, &str)]) -> Graph {
        let mut graph = Graph::new();
        for (path, name) in pieces {
            graph.insert(item(&format!("{WARDROBE}{path}"), name));
        }
        graph
    }

    #[test]
    fn the_operators_piece_points_at_the_drifters() {
        let mut graph = wardrobe(&[
            ("Sleeves/SleevesDaxB", "Lark Bishamo Pauldrons"),
            ("Sleeves/SleevesAdultDaxB", "Lark Bishamo Pauldrons"),
        ]);
        assert_eq!(link(&mut graph), 1);

        let out = graph.from(&format!("{WARDROBE}Sleeves/SleevesDaxB"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].to, format!("{WARDROBE}Sleeves/SleevesAdultDaxB"));
        assert_eq!(out[0].rel, Rel::Fits);
    }

    /// Armour has no Drifter slot, so the Drifter's copy of a piece of armour is a sleeve.
    #[test]
    fn the_two_halves_need_not_sit_in_the_same_folder() {
        let mut graph = wardrobe(&[
            ("Armour/Teshin/TeshinArmourArms", "Hawk Bishamo Pauldrons"),
            ("Sleeves/SleevesAdultDaxA", "Hawk Bishamo Pauldrons"),
        ]);
        assert_eq!(link(&mut graph), 1);
    }

    #[test]
    fn two_pieces_of_one_wardrobe_are_not_a_pair_by_themselves() {
        let mut graph = wardrobe(&[
            ("FacialMarkings/FacialMarkingA", "Somatics D11"),
            ("FacialMarkings/NewFacialMarkingA", "Somatics D11"),
        ]);
        assert_eq!(link(&mut graph), 0);
    }

    #[test]
    fn a_name_shared_by_more_than_two_pieces_is_left_alone() {
        let mut graph = wardrobe(&[
            ("Sleeves/SleevesX", "Ambiguous"),
            ("Sleeves/SleevesAdultX", "Ambiguous"),
            ("Sleeves/SleevesAdultY", "Ambiguous"),
        ]);
        assert_eq!(link(&mut graph), 0);
    }
}
