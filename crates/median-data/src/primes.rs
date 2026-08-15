use std::collections::BTreeMap;

use graph::{Edge, Graph, Rel};

/// Link every prime item to the ordinary item it upgrades. A prime's name is the ordinary
/// name with `Prime` inserted, so the base is found by removing the word; the match must be
/// unique and in the same category. Primes the game never shipped a plain version of —
/// Dakra Prime, Reaper Prime — get no edge, and that absence is the fact.
pub fn link(graph: &mut Graph) -> usize {
    let mut plain: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for item in graph.items() {
        if item.prime.value || graph::tiered(&item.unique_name) {
            continue;
        }
        plain
            .entry((item.names.en.value.as_str(), item.category.value.as_str()))
            .or_default()
            .push(item.unique_name.as_str());
    }

    let mut edges = Vec::new();
    for item in graph.items() {
        if !item.prime.value {
            continue;
        }
        let base = without_prime(&item.names.en.value);
        let key = (base.as_str(), item.category.value.as_str());
        if let Some([only]) = plain.get(&key).map(Vec::as_slice) {
            edges.push(Edge {
                from: only.to_string(),
                to: item.unique_name.clone(),
                rel: Rel::Primed,
            });
        }
    }

    let count = edges.len();
    for edge in edges {
        graph.link(edge);
    }
    count
}

/// The ordinary name behind a prime one: the same words without `Prime`.
fn without_prime(name: &str) -> String {
    name.split_whitespace()
        .filter(|w| *w != "Prime")
        .collect::<Vec<_>>()
        .join(" ")
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

    fn item(path: &str, name: &str, category: &str, prime: bool) -> Node {
        Node::Item(Item {
            unique_name: path.into(),
            names: Names {
                en: value(name.to_string()),
                ru: None,
            },
            category: value(category.to_string()),
            kind: value(graph::Kind::unknown()),
            slug: None,
            tradable: None,
            vaulted: None,
            prime: value(prime),
            ducats: None,
            extra: Extra::None,
        })
    }

    #[test]
    fn a_prime_points_back_at_its_plain_version() {
        let mut graph = Graph::new();
        graph.insert(item(
            "/Weapons/Tenno/Pistol/HeavyPistol",
            "Lex",
            "Pistols",
            false,
        ));
        graph.insert(item(
            "/Weapons/Tenno/Pistol/LexPrime",
            "Lex Prime",
            "Pistols",
            true,
        ));
        assert_eq!(link(&mut graph), 1);

        let out = graph.from("/Weapons/Tenno/Pistol/HeavyPistol");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].to, "/Weapons/Tenno/Pistol/LexPrime");
        assert_eq!(out[0].rel, Rel::Primed);
    }

    #[test]
    fn a_prime_only_weapon_gets_no_edge() {
        let mut graph = Graph::new();
        graph.insert(item("/Weapons/DakraPrime", "Dakra Prime", "Melee", true));
        assert_eq!(link(&mut graph), 0);
    }

    #[test]
    fn a_different_category_is_not_the_base() {
        let mut graph = Graph::new();
        graph.insert(item("/D/AshNoggle", "Ash", "Decoration", false));
        graph.insert(item("/Powersuits/AshPrime", "Ash Prime", "Suits", true));
        assert_eq!(link(&mut graph), 0);
    }
}
