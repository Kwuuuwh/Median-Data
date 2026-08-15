use std::collections::{BTreeMap, BTreeSet};

use graph::{Edge, Graph, Node, Offer, Rel, Vendor, vendor_id};

use crate::names::{self, Index};
use crate::orphans::{self, Missing};
use crate::wiki::{Offered, Store};

/// Baro Ki'Teer, whose whole stock changes every visit. He is the one vendor with his own
/// module, because the wiki keeps the history of what he brought and when.
const BARO: (&str, &str, &str) = ("baro", "Baro Ki'Teer", "Баро Ки'Тир");

/// His key and the wiki page that illustrates him.
pub const BARO_PAGE: (&str, &str) = ("baro", "Baro Ki'Teer");

/// What tying vendor offerings to the catalog produced.
pub struct Linked {
    pub vendors: usize,
    pub offers: usize,
    /// Names no catalog item answers to: bundles, boosters, armour sets sold as one thing.
    pub unresolved: Missing,
}

impl Linked {
    fn new() -> Self {
        Self {
            vendors: 0,
            offers: 0,
            unresolved: Missing::new(),
        }
    }

    fn absorb(&mut self, other: Linked) {
        self.vendors += other.vendors;
        self.offers += other.offers;
        for (name, seen) in other.unresolved {
            let mine = self.unresolved.entry(name).or_default();
            mine.count += seen.count;
            if mine.hint.is_empty() {
                mine.hint = seen.hint;
            }
        }
    }
}

/// Every vendor: the ones the wiki lists by stock, and Baro from his own history.
pub fn link(
    graph: &mut Graph,
    stores: &[Store],
    baro_stock: &[Offered],
    index: &Index,
    terms: &crate::curation::Terms,
) -> Linked {
    let mut out = Linked::new();
    out.absorb(stores_of(graph, stores, index, terms));
    out.absorb(baro(graph, baro_stock, index, terms));
    out
}

/// The vendors of `Module:Vendors/data`. The module is keyed by counter, not by person — Yonta
/// keeps two, one taking Thrax Plasm and one taking Voidplume Pinions — so entries are grouped
/// by the wiki page they link to, which is who the vendor is. The counter and its currency move
/// onto the offer, where they belong. An edge says the vendor hands the item over; a `timer` is
/// what rotates, and nothing here claims what is in stock at this moment.
fn stores_of(
    graph: &mut Graph,
    stores: &[Store],
    index: &Index,
    terms: &crate::curation::Terms,
) -> Linked {
    let mut out = Linked::new();
    let mut counters: BTreeMap<String, Vec<&Store>> = BTreeMap::new();
    for store in stores {
        counters.entry(person(store)).or_default().push(store);
    }

    for (page, group) in counters {
        let key = slug(&page);
        // One counter keeps its own name; several are one person, and the page names them.
        let name = match group.as_slice() {
            [only] => only.name.clone(),
            _ => page.clone(),
        };
        let currencies: BTreeSet<&str> =
            group.iter().filter_map(|s| s.currency.as_deref()).collect();
        if graph.insert(Node::Vendor(Vendor {
            key: key.clone(),
            name: name.clone(),
            name_ru: terms.get("vendor", &key).map(str::to_string),
            currency: match currencies.len() {
                1 => currencies.iter().next().map(|c| c.to_string()),
                _ => None,
            },
            kind: group.iter().find_map(|s| s.kind.clone()),
            rotates: false,
        })) {
            out.vendors += 1;
        }
        let from = vendor_id(&key);

        let mut edges = Vec::new();
        for store in &group {
            let counter = (store.name != name).then(|| store.name.clone());
            for offer in &store.offers {
                let (_, printed) = names::quantity(&offer.name);
                let Some(item) = resolve(index, printed, &offer.kind) else {
                    orphans::note(&mut out.unresolved, printed, || from.clone());
                    continue;
                };
                edges.push(Edge {
                    from: from.clone(),
                    to: item.to_string(),
                    rel: Rel::Sells(Offer {
                        cost: Some(offer.cost).filter(|c| *c > 0),
                        currency: store.currency.clone(),
                        store: counter.clone(),
                        credits: None,
                        count: offer.count,
                        rank: offer.rank,
                        timer: offer.timer,
                        times: 0,
                        always: false,
                        gone: false,
                    }),
                });
            }
        }
        out.offers += edges.len();
        for edge in edges {
            graph.link(edge);
        }
    }
    out
}

/// Who a counter belongs to: the wiki page it links to, without the section anchor and without
/// the disambiguator the wiki adds to a page title (`Vox Solaris (Syndicate)`, `Loid (Original)`).
pub fn person(store: &Store) -> String {
    let link = store.link.as_deref().unwrap_or(&store.name);
    let page = link.split('#').next().unwrap_or(link).replace('_', " ");
    match page.split_once(" (") {
        Some((head, _)) if !head.is_empty() => head.to_string(),
        _ => page,
    }
}

/// Baro and everything he has ever brought. The edge says the item has been offered, not that
/// it is on sale: his stock rotates, and what stands in his kiosk today is live world state
/// that a pinned catalog cannot answer for.
fn baro(
    graph: &mut Graph,
    offered: &[Offered],
    index: &Index,
    terms: &crate::curation::Terms,
) -> Linked {
    let (key, name, name_ru) = BARO;
    let mut out = Linked::new();

    if graph.insert(Node::Vendor(Vendor {
        key: key.to_string(),
        name: name.to_string(),
        name_ru: terms.or("vendor", key, Some(name_ru.to_string())),
        currency: Some("Ducats".to_string()),
        kind: Some("Store".to_string()),
        rotates: true,
    })) {
        out.vendors += 1;
    }
    let from = vendor_id(key);

    for it in offered {
        let (_, printed) = names::quantity(&it.name);
        let Some(item) = index.get(printed) else {
            orphans::note(&mut out.unresolved, printed, || from.clone());
            continue;
        };
        out.offers += 1;
        graph.link(Edge {
            from: from.clone(),
            to: item.to_string(),
            rel: Rel::Sells(Offer {
                cost: it.ducats,
                currency: Some("Ducats".to_string()),
                store: None,
                credits: it.credits,
                count: 1,
                rank: None,
                timer: None,
                times: it.times,
                always: it.always,
                gone: it.gone,
            }),
        });
    }
    out
}

/// A stock line resolved to a catalog path. The wiki names a relic without the word and says
/// what it is in its own kind column, so the kind is what puts the word back.
fn resolve<'a>(index: &'a Index, printed: &str, kind: &str) -> Option<&'a str> {
    if kind == "Relic"
        && let Some(path) = index.get(&format!("{printed} Relic"))
    {
        return Some(path);
    }
    index.get(printed)
}

/// A node key from a vendor's name: lower case, spaces and punctuation folded to dashes.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vendor_key_survives_punctuation() {
        assert_eq!(slug("Cephalon Simaris"), "cephalon-simaris");
        assert_eq!(slug("Kahl's Garrison"), "kahl-s-garrison");
        assert_eq!(slug("The Perrin Sequence"), "the-perrin-sequence");
    }
}
