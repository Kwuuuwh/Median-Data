use std::collections::{BTreeMap, BTreeSet};

use graph::{Edge, Graph, Node, Offer, Rel, Vendor, vendor_id};

use crate::curation::{Curation, Terms};
use crate::names::{self, Index};
use crate::orphans::{self, Missing};
use crate::wiki::{Offered, Priced, Store};

/// Baro Ki'Teer, whose whole stock changes every visit. He is the one vendor with his own
/// module, because the wiki keeps the history of what he brought and when.
const BARO: (&str, &str, &str) = ("baro", "Baro Ki'Teer", "Баро Ки'Тир");

/// His key and the wiki page that illustrates him.
pub const BARO_PAGE: (&str, &str) = ("baro", "Baro Ki'Teer");

/// The wiki page of the in-game market.
const MARKET: &str = "Market";

/// The one name of Nightwave's currency, whatever season prints on it.
const CRED: &str = "Cred";

/// The kind of counter whose stock rotates on a schedule.
const ROTATING: &str = "Rotating Store";

/// Kinds of counter whose stock comes and goes: a rotation, an event, a Nightwave season.
const FLEETING: [&str; 3] = [ROTATING, "Event", "Nightwave"];

/// Everything the vendor sources hand a build.
pub struct Stock<'a> {
    pub stores: &'a [Store],
    pub baro: &'a [Offered],
    /// Blueprints the market sells for credits.
    pub market: &'a [Priced],
}

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

/// Every vendor: the ones the wiki lists by stock, Baro from his own history, the market's
/// blueprints, and what was written by hand.
pub fn link(
    graph: &mut Graph,
    stock: &Stock,
    index: &Index,
    curated: &Curation,
    terms: &Terms,
) -> Linked {
    let prices = curated.prices();
    let mut out = Linked::new();
    out.absorb(stores_of(graph, stock.stores, index, terms, &prices));
    out.absorb(baro(graph, stock.baro, index, terms));
    out.absorb(market(graph, stock.market, index, terms));
    out.absorb(curated_of(graph, stock.stores, index, curated));
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
    terms: &Terms,
    prices: &BTreeMap<(&str, &str), i64>,
) -> Linked {
    let mut out = Linked::new();
    for (page, group) in counters(stores) {
        let key = slug(&page);
        let kind = group.iter().find_map(|s| s.kind.clone());
        let fleeting = kind.as_deref().is_some_and(|kind| FLEETING.contains(&kind));
        let currencies: BTreeSet<String> = group
            .iter()
            .filter_map(|s| s.currency.as_deref())
            .map(currency)
            .collect();
        if graph.insert(Node::Vendor(Vendor {
            key: key.clone(),
            name: page.clone(),
            name_ru: terms.get("vendor", &key).map(str::to_string),
            currency: match currencies.len() {
                1 => currencies.into_iter().next(),
                _ => None,
            },
            rotates: kind.as_deref() == Some(ROTATING),
            kind,
        })) {
            out.vendors += 1;
        }
        let from = vendor_id(&key);

        let mut edges = Vec::new();
        for store in &group {
            let counter = (store.name != page).then(|| store.name.clone());
            for offer in &store.offers {
                let (_, printed) = names::quantity(&offer.name);
                let Some(item) = resolve(index, printed, &offer.kind) else {
                    orphans::note(&mut out.unresolved, printed, || from.clone());
                    continue;
                };
                let cost = prices
                    .get(&(key.as_str(), printed))
                    .copied()
                    .unwrap_or(offer.cost);
                let currency = offer
                    .currency
                    .as_deref()
                    .or(store.currency.as_deref())
                    .map(currency);
                edges.push(Edge {
                    from: from.clone(),
                    to: item.to_string(),
                    rel: Rel::Sells(Offer {
                        cost: Some(cost).filter(|c| *c > 0),
                        pays: paid_in(index, currency.as_deref()),
                        currency,
                        store: counter.clone(),
                        credits: offer.credits,
                        count: offer.count,
                        rank: offer.rank,
                        timer: offer.timer,
                        times: 0,
                        always: offer.timer.is_none() && !fleeting,
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

/// Counters grouped by the person who keeps them.
fn counters(stores: &[Store]) -> BTreeMap<String, Vec<&Store>> {
    let mut counters: BTreeMap<String, Vec<&Store>> = BTreeMap::new();
    for store in stores {
        counters.entry(person(store)).or_default().push(store);
    }
    counters
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

/// The catalog item a currency is, where it is one: standing and platinum are not.
fn paid_in(index: &Index, currency: Option<&str>) -> Option<String> {
    currency
        .and_then(|currency| index.get(currency))
        .map(str::to_string)
}

/// A currency under the name the game gives it everywhere.
fn currency(printed: &str) -> String {
    match printed.ends_with(" Cred") {
        true => CRED.to_string(),
        false => printed.to_string(),
    }
}

/// Baro and everything he has ever brought. The edge says the item has been offered, not that
/// it is on sale: his stock rotates, and what stands in his kiosk today is live world state
/// that a pinned catalog cannot answer for.
fn baro(graph: &mut Graph, offered: &[Offered], index: &Index, terms: &Terms) -> Linked {
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
                pays: paid_in(index, Some("Ducats")),
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

/// Blueprints the market sells for credits, which is all it charges.
fn market(graph: &mut Graph, priced: &[Priced], index: &Index, terms: &Terms) -> Linked {
    let key = slug(MARKET);
    let mut out = Linked::new();
    if graph.insert(Node::Vendor(Vendor {
        key: key.clone(),
        name: MARKET.to_string(),
        name_ru: terms.get("vendor", &key).map(str::to_string),
        currency: None,
        kind: Some("Store".to_string()),
        rotates: false,
    })) {
        out.vendors += 1;
    }
    let from = vendor_id(&key);

    for blueprint in priced {
        let Some(item) = index.get(&blueprint.name) else {
            orphans::note(&mut out.unresolved, &blueprint.name, || from.clone());
            continue;
        };
        out.offers += 1;
        graph.link(Edge {
            from: from.clone(),
            to: item.to_string(),
            rel: Rel::Sells(Offer {
                cost: None,
                currency: None,
                pays: None,
                credits: Some(blueprint.credits),
                ..offer()
            }),
        });
    }
    out
}

/// Vendors and offers written by hand, for what no source lists; each is a stall that stays.
/// A hand-written price for a line a source does list is applied where that line is read.
fn curated_of(graph: &mut Graph, stores: &[Store], index: &Index, curated: &Curation) -> Linked {
    let mut out = Linked::new();
    for seller in &curated.vendor {
        if graph.insert(Node::Vendor(Vendor {
            key: seller.key.clone(),
            name: seller.name.clone(),
            name_ru: Some(seller.ru.clone()),
            currency: Some(seller.currency.clone()),
            kind: Some("Store".to_string()),
            rotates: false,
        })) {
            out.vendors += 1;
        }
    }

    let listed: BTreeSet<(String, &str)> = counters(stores)
        .into_iter()
        .flat_map(|(page, group)| {
            let key = slug(&page);
            group
                .into_iter()
                .flat_map(|store| &store.offers)
                .map(move |offer| (key.clone(), names::quantity(&offer.name).1))
        })
        .collect();
    for sale in &curated.offer {
        if listed.contains(&(sale.vendor.clone(), sale.item.as_str())) {
            continue;
        }
        let from = vendor_id(&sale.vendor);
        let (Some(item), Some(Node::Vendor(vendor))) = (index.get(&sale.item), graph.get(&from))
        else {
            orphans::note(&mut out.unresolved, &sale.item, || from.clone());
            continue;
        };
        let (item, currency) = (
            item.to_string(),
            vendor.currency.clone().unwrap_or_default(),
        );
        out.offers += 1;
        graph.link(Edge {
            from: from.clone(),
            to: item,
            rel: Rel::Sells(Offer {
                cost: Some(sale.cost),
                pays: paid_in(index, Some(&currency)),
                currency: Some(currency),
                ..offer()
            }),
        });
    }
    out
}

/// A plain offer: one copy, always on the counter, its price still to be said.
fn offer() -> Offer {
    Offer {
        cost: None,
        currency: None,
        pays: None,
        store: None,
        credits: None,
        count: 1,
        rank: None,
        timer: None,
        times: 0,
        always: true,
        gone: false,
    }
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
    use crate::wiki::StoreOffer;
    use consensus::{Claim, Resolved, Source, resolve};
    use graph::{Extra, Item, Names};

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

    fn catalog(names: &[(&str, &str)]) -> Graph {
        let mut graph = Graph::new();
        for (path, name) in names {
            graph.insert(Node::Item(Item {
                unique_name: (*path).into(),
                names: Names {
                    en: value(name.to_string()),
                    ru: None,
                },
                category: value("Test".to_string()),
                kind: value(graph::Kind::unknown()),
                slug: None,
                tradable: None,
                vaulted: None,
                prime: value(false),
                ducats: None,
                mastery: None,
                mastery_req: None,
                max_level_cap: None,
                extra: Extra::None,
            }));
        }
        graph
    }

    fn selling(name: &str, kind: &str, currency: &str, item: &str, timer: Option<i64>) -> Store {
        Store {
            name: name.to_string(),
            link: Some(name.to_string()),
            currency: Some(currency.to_string()),
            kind: Some(kind.to_string()),
            offers: vec![StoreOffer {
                name: item.to_string(),
                kind: "Weapon".to_string(),
                cost: 50,
                currency: None,
                credits: None,
                count: 1,
                rank: None,
                timer,
            }],
        }
    }

    /// The one offer the vendor makes.
    fn sold(graph: &Graph, vendor: &str) -> Offer {
        graph
            .from(&vendor_id(vendor))
            .into_iter()
            .find_map(|edge| match &edge.rel {
                Rel::Sells(offer) => Some(offer.clone()),
                _ => None,
            })
            .expect("an offer")
    }

    #[test]
    fn an_offer_names_the_item_its_currency_is_and_stays_on_a_plain_counter() {
        let mut graph = catalog(&[
            ("/Clamp", "Pathos Clamp"),
            ("/SyamBlueprint", "Syam Blueprint"),
        ]);
        let index = Index::build(&graph, &BTreeMap::new());
        let stores = [
            selling("Teshin", "Store", "Pathos Clamp", "Syam Blueprint", None),
            selling(
                "Acrithis",
                "Store",
                "Pathos Clamp",
                "Syam Blueprint",
                Some(604_800),
            ),
            selling("Palladino", ROTATING, "Standing", "Syam Blueprint", None),
        ];

        stores_of(
            &mut graph,
            &stores,
            &index,
            &Curation::default().terms(&BTreeMap::new()),
            &BTreeMap::new(),
        );

        let teshin = sold(&graph, "teshin");
        assert_eq!(teshin.pays.as_deref(), Some("/Clamp"));
        assert!(teshin.always);
        assert!(!sold(&graph, "acrithis").always);
        let palladino = sold(&graph, "palladino");
        assert_eq!(palladino.pays, None);
        assert!(!palladino.always);
        assert!(matches!(
            graph.get(&vendor_id("palladino")),
            Some(Node::Vendor(Vendor { rotates: true, .. }))
        ));
    }

    #[test]
    fn the_market_charges_credits_and_nothing_else() {
        let mut graph = catalog(&[("/BratonBlueprint", "Braton Blueprint")]);
        let index = Index::build(&graph, &BTreeMap::new());
        let priced = [Priced {
            name: "Braton Blueprint".to_string(),
            credits: 1_500,
        }];

        market(
            &mut graph,
            &priced,
            &index,
            &Curation::default().terms(&BTreeMap::new()),
        );

        let offer = sold(&graph, &slug(MARKET));
        assert_eq!((offer.cost, offer.credits), (None, Some(1_500)));
        assert!(offer.currency.is_none() && offer.pays.is_none() && offer.always);
    }

    fn store(name: &str, link: &str) -> Store {
        Store {
            name: name.to_string(),
            link: Some(link.to_string()),
            currency: None,
            kind: None,
            offers: Vec::new(),
        }
    }

    #[test]
    fn a_vendor_key_survives_punctuation() {
        assert_eq!(slug("Cephalon Simaris"), "cephalon-simaris");
        assert_eq!(slug("Kahl's Garrison"), "kahl-s-garrison");
        assert_eq!(slug("The Perrin Sequence"), "the-perrin-sequence");
    }

    #[test]
    fn a_lone_counter_is_filed_under_its_keeper() {
        let stores = [store("Release Vestigal Motes", "Ordis#Jade Shadows")];
        let grouped = counters(&stores);
        assert_eq!(grouped.keys().collect::<Vec<_>>(), ["Ordis"]);
    }

    #[test]
    fn every_nightwave_season_pays_in_cred() {
        assert_eq!(currency("Nora's Mix Vol. 6 Cred"), "Cred");
        assert_eq!(currency("Standing"), "Standing");
    }
}
