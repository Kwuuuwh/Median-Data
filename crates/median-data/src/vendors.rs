use std::collections::{BTreeMap, BTreeSet};

use graph::{Edge, Graph, Node, Offer, Rel, Vendor, vendor_id};

use crate::curation::{Curation, Terms};
use crate::names::{self, Index};
use crate::normalize;
use crate::offers::Stall;
use crate::orphans::{self, Missing};
use crate::wiki::{Offered, Priced};

/// Baro Ki'Teer, whose whole stock changes every visit. He is the one vendor with his own
/// module, because the wiki keeps the history of what he brought and when.
const BARO: (&str, &str, &str) = ("baro", "Baro Ki'Teer", "Баро Ки'Тир");

/// His key and the wiki page that illustrates him.
pub const BARO_PAGE: (&str, &str) = ("baro", "Baro Ki'Teer");

/// The wiki page of the in-game market.
const MARKET: &str = "Market";

/// Where the game keeps a vendor's manifest, and the area of the catalog that is. The
/// directory is the hub the counter stands in: Ostron manifests are Cetus, Solaris ones are
/// Fortuna, Deimos ones the Necralisk, and Albrecht's labs are the Sanctum beneath it.
const HUBS: [(&str, &str); 8] = [
    ("Deimos", "necralisk"),
    ("Duviri", "duviri"),
    ("EntratiLab", "sanctum"),
    ("EntratiLabs", "sanctum"),
    ("Hex", "hollvania"),
    ("Ostron", "cetus"),
    ("Solaris", "fortuna"),
    ("TheHex", "hollvania"),
];

/// Zariman keeps the same name as its area.
const ZARIMAN: &str = "Zariman";

/// Where the game keeps the adapters a stall sells as a package, and the word the package
/// ends with. `…/Packages/IncarnonPackages/BoarIncarnonBundle` is the package around
/// `…/IncarnonAdapters/Primary/BoarIncarnonUnlocker`, and nothing but the leaf ties them.
const ADAPTERS: &str = "/IncarnonAdapters/";
const PACKAGED: (&str, &str) = ("Bundle", "Unlocker");

/// Offers whose nature is not an item at all. The stall takes payment for every one of them,
/// but what changes hands is a job, an exchange, a crew member or several items at once, so
/// no single catalog item can stand in for it. Read in order: the bundle rule at the end
/// only speaks for packages the ones above it did not name.
const NOT_AN_ITEM: [(&str, &str); 10] = [
    ("/Packages/Tasks/", "a job the family's stall arranges"),
    ("/Packages/DebtTokenBundles/", "a debt-token exchange"),
    ("/CrewShip/CrewMember/", "a railjack crew member"),
    ("/CrewMembers/", "a crew member"),
    ("/Types/Items/Guild/GuildAdvertisement", "a clan advert"),
    ("/Types/BoosterPacks/", "a pack of random things"),
    ("/StoreItems/CreditBundles/", "credits"),
    ("/StoreItems/SlotItems/", "a slot"),
    ("/Mods/FusionBundles/", "endo"),
    ("/StoreItems/Packages/", "several items sold as one"),
];

/// Trades the game files a counter under. One person keeps them all: the Cetus syndicate
/// counter tagged `Fishmonger` and the Ostron `FishmongerVendorManifest` are both Hai-Luk.
const TRADES: [&str; 6] = [
    "Weaponsmith",
    "Fishmonger",
    "Prospector",
    "PetVendor",
    "MoaVendor",
    "ConservationRewards",
];

/// How much of a counter's stock has to match a wiki vendor's before it is the same person.
const SURE_SHARE: f64 = 0.9;
const SURE_SHARED: usize = 3;

/// The two currencies that are not items.
const STANDING: &str = "Standing";
const PLATINUM: &str = "Platinum";
const CREDITS: &str = "Credits";

/// Where the game keeps the syndicates, whose stalls are filed as such.
const SYNDICATES: &str = "/Lotus/Syndicates/";

/// What a stall is, where nothing finer is known.
const STORE: &str = "Store";
const SYNDICATE: &str = "Syndicate";

/// Nightwave keeps a manifest per season; they are all the same counter.
const NIGHTWAVE: (&str, &str) = ("nightwave", "Nightwave");

/// The path every Nightwave season's manifest starts its name with.
const SEASON: &str = "radio-legion";

/// Everything the vendor sources hand a build.
pub struct Stock<'a> {
    /// What the game's own manifests sell.
    pub stalls: &'a [Stall],
    /// Who the wiki says keeps which counter, by what it lists them selling.
    pub keepers: &'a [crate::wiki::Store],
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
    /// Offers that answer to no item because they are not one, by what they are instead.
    pub not_items: BTreeMap<&'static str, usize>,
}

impl Linked {
    fn new() -> Self {
        Self {
            vendors: 0,
            offers: 0,
            unresolved: Missing::new(),
            not_items: BTreeMap::new(),
        }
    }

    fn absorb(&mut self, other: Linked) {
        self.vendors += other.vendors;
        self.offers += other.offers;
        for (why, count) in other.not_items {
            *self.not_items.entry(why).or_default() += count;
        }
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
    let named = keepers(stock.stalls, stock.keepers, index);
    let mut out = Linked::new();
    out.absorb(stalls_of(graph, stock.stalls, terms, &prices, &named));
    out.absorb(baro(graph, stock.baro, index, terms));
    out.absorb(market(graph, stock.market, index, terms));
    out.absorb(curated_of(graph, stock.stalls, index, curated));
    out
}

/// What the game's own manifests sell. A manifest names no person, so a vendor is the
/// manifest itself — or, where offers stand on named counters, the manifest and the counter:
/// Cetus keeps four, and the game calls them Weaponsmith, Fishmonger, Prospector, PetVendor.
fn stalls_of(
    graph: &mut Graph,
    stalls: &[Stall],
    terms: &Terms,
    prices: &BTreeMap<(&str, &str), i64>,
    named: &BTreeMap<String, String>,
) -> Linked {
    let mut out = Linked::new();
    let keys = keys(stalls);
    let current = current_season(stalls);
    // A season that has closed sells what the running one sells, so the running one is read
    // first and the closed ones only add what nobody offers any more.
    let mut ordered: Vec<&Stall> = stalls.iter().collect();
    ordered.sort_by_key(|stall| season(&stall.manifest).is_some_and(|s| Some(s) != current));
    let mut linked: BTreeSet<(String, String)> = BTreeSet::new();
    let mut standing: BTreeSet<(String, &str)> = BTreeSet::new();
    let agreed = agreed_keepers(stalls, &keys, named);
    let same = same_person(stalls, &keys, &agreed, terms);
    let adapters = adapters(graph);

    for stall in ordered {
        let base = &keys[stall.manifest.as_str()];
        let gone = season(&stall.manifest).is_some_and(|season| Some(season) != current);
        for offer in &stall.offer {
            let item = normalize::path(&offer.item).into_owned();
            let item = match graph.has(&item) {
                true => item,
                false => match packaged(&item, &adapters) {
                    Some(item) => item,
                    None => {
                        match not_an_item(&item) {
                            Some(why) => *out.not_items.entry(why).or_default() += 1,
                            None => {
                                orphans::note(&mut out.unresolved, &offer.item, || vendor_id(base))
                            }
                        }
                        continue;
                    }
                },
            };
            let counter = counter_key(base, offer, &stall.manifest);
            // A person keeps every counter of theirs under one key: Hai-Luk takes bait on one
            // tab of her stall and fishing gear on another, and she is one vendor.
            let (key, name) = match agreed.get(&counter) {
                Some(keeper) => (slug(keeper), keeper.clone()),
                None => (counter.clone(), title(&counter)),
            };
            let (key, name) = match same.get(&key) {
                Some(main) => (main.clone(), title(main)),
                None => (key, name),
            };
            if !linked.insert((key.clone(), item.clone())) {
                continue;
            }
            let kind = match stall.manifest.starts_with(SYNDICATES) {
                true => SYNDICATE,
                false => STORE,
            };
            if let Some(area) = hub_of(&stall.manifest) {
                standing.insert((key.clone(), area));
            }
            if graph.insert(Node::Vendor(Vendor {
                name_ru: terms.of("vendor", &key, &name),
                name,
                currency: None,
                rotates: stall.floating,
                kind: Some(kind.to_string()),
                key: key.clone(),
            })) {
                out.vendors += 1;
            }
            let asked = asked(graph, offer);
            let cost = prices
                .get(&(key.as_str(), item.as_str()))
                .copied()
                .or(asked.cost);
            out.offers += 1;
            graph.link(Edge {
                from: vendor_id(&key),
                to: item,
                rel: Rel::Sells(Offer {
                    cost,
                    cost_max: asked.cost_max,
                    floating: stall.floating || asked.floating,
                    currency: asked.currency,
                    pays: asked.pays,
                    also: asked.also,
                    store: offer.store.clone(),
                    credits: offer.credits.map(|credits| credits[0]).filter(|c| *c > 1),
                    limit: offer.limit,
                    count: offer.count.unwrap_or(1),
                    rank: offer.rank,
                    timer: offer.hours.map(|hours| hours[0] * 3600),
                    times: 0,
                    always: offer.always,
                    gone,
                }),
            });
        }
    }
    for (vendor, area) in standing {
        graph.link(Edge {
            from: vendor_id(&vendor),
            to: graph::area_id(area),
            rel: Rel::Within,
        });
    }
    name_currencies(graph);
    out
}

/// The area a manifest's directory names, where it names one.
fn hub_of(manifest: &str) -> Option<&'static str> {
    let folder = manifest.rsplit_once('/')?.0.rsplit('/').next()?;
    if folder == ZARIMAN {
        return Some("zariman");
    }
    HUBS.iter()
        .find(|(directory, _)| *directory == folder)
        .map(|(_, area)| *area)
}

/// A vendor takes one currency where every offer of theirs is counted in the same thing.
fn name_currencies(graph: &mut Graph) {
    let mut taken: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in graph.edges() {
        let Rel::Sells(offer) = &edge.rel else {
            continue;
        };
        let Some(currency) = &offer.currency else {
            continue;
        };
        taken
            .entry(edge.from.clone())
            .or_default()
            .insert(currency.clone());
    }
    for (id, currencies) in taken {
        if currencies.len() != 1 {
            continue;
        }
        let only = currencies.into_iter().next().expect("one currency");
        if let Some(Node::Vendor(vendor)) = graph.get_mut(&id) {
            vendor.currency = Some(only);
        }
    }
}

/// The counter an offer stands on, as the game files it.
fn raw_key(base: &str, offer: &crate::offers::Offer) -> String {
    match &offer.store {
        Some(counter) => format!("{base}-{}", dashed(counter)),
        None => base.to_string(),
    }
}

/// The vendor an offer belongs to: the manifest's keeper, and the counter where there is one.
/// A counter named after a trade belongs to whoever plies that trade in the hub, however many
/// manifests the game splits their wares across.
fn counter_key(base: &str, offer: &crate::offers::Offer, manifest: &str) -> String {
    let trade = offer
        .store
        .as_deref()
        .and_then(trade_of)
        .or_else(|| manifest_trade(manifest));
    match (trade, hub_of(manifest)) {
        (Some(trade), Some(hub)) => format!("{hub}-{}", dashed(trade)),
        (Some(trade), None) => format!("{base}-{}", dashed(trade)),
        (None, _) => match &offer.store {
            Some(counter) => format!("{base}-{}", dashed(counter)),
            None => base.to_string(),
        },
    }
}

/// The trade a counter's name is, where it is one.
fn trade_of(name: &str) -> Option<&'static str> {
    TRADES
        .iter()
        .find(|trade| name.eq_ignore_ascii_case(trade) || name.ends_with(*trade))
        .copied()
}

/// The trade a manifest is named after. The game ends these names two ways — `PetVendor`
/// plus `Manifest`, `Fishmonger` plus `VendorManifest` — so both readings are tried.
fn manifest_trade(manifest: &str) -> Option<&'static str> {
    let tail = manifest.rsplit('/').next()?;
    let full = tail.strip_suffix("Manifest").unwrap_or(tail);
    trade_of(split(manifest).1).or_else(|| trade_of(full))
}

/// Who a counter belongs to: the wiki page it links to, without the section anchor and
/// without the disambiguator the wiki adds to a page title (`Vox Solaris (Syndicate)`).
/// The page is who the person is, and it is what illustrates them.
pub fn person(store: &crate::wiki::Store) -> String {
    let link = store.link.as_deref().unwrap_or(&store.name);
    let page = link.split('#').next().unwrap_or(link).replace('_', " ");
    match page.split_once(" (") {
        Some((head, _)) if !head.is_empty() => head.to_string(),
        _ => page,
    }
}

/// Counters that answer to the same Russian name. A manifest splits a vendor by tab — Biz
/// takes fish at one counter and sells gear at another, and no shared stock ties the two —
/// so the name is what says they are one person. The first key alphabetically keeps them.
fn same_person(
    stalls: &[Stall],
    keys: &BTreeMap<&str, String>,
    agreed: &BTreeMap<String, String>,
    terms: &Terms,
) -> BTreeMap<String, String> {
    let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for stall in stalls {
        let base = &keys[stall.manifest.as_str()];
        for offer in &stall.offer {
            let counter = counter_key(base, offer, &stall.manifest);
            let (key, name) = match agreed.get(&counter) {
                Some(keeper) => (slug(keeper), keeper.clone()),
                None => (counter.clone(), title(&counter)),
            };
            if let Some(ru) = terms.of("vendor", &key, &name) {
                named.entry(ru).or_default().insert(key);
            }
        }
    }
    named
        .into_values()
        .filter(|keys| keys.len() > 1)
        .flat_map(|keys| {
            let main = keys.iter().next().cloned().expect("a key");
            keys.into_iter().map(move |key| (key, main.clone()))
        })
        .collect()
}

/// The keeper of each merged counter, where every counter merged into it names the same
/// person. Two people never share a key, so a disagreement leaves the counter unnamed.
fn agreed_keepers(
    stalls: &[Stall],
    keys: &BTreeMap<&str, String>,
    named: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut claims: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for stall in stalls {
        let base = &keys[stall.manifest.as_str()];
        for offer in &stall.offer {
            let Some(keeper) = named.get(&raw_key(base, offer)) else {
                continue;
            };
            claims
                .entry(counter_key(base, offer, &stall.manifest))
                .or_default()
                .insert(keeper.as_str());
        }
    }
    claims
        .into_iter()
        .filter(|(_, keepers)| keepers.len() == 1)
        .map(|(counter, keepers)| {
            let keeper = keepers.into_iter().next().expect("one keeper");
            (counter, keeper.to_string())
        })
        .collect()
}

/// Who keeps each counter. A manifest names no person, so the person is the wiki vendor
/// whose listed stock the manifest sells: the wiki knows Hok keeps the anvil in Cetus, the
/// cache knows what the anvil sells today, and the two meet on the items themselves.
fn keepers(
    stalls: &[Stall],
    listed: &[crate::wiki::Store],
    index: &Index,
) -> BTreeMap<String, String> {
    let keys = keys(stalls);
    let mut sold: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for stall in stalls {
        let base = &keys[stall.manifest.as_str()];
        for offer in &stall.offer {
            sold.entry(raw_key(base, offer))
                .or_default()
                .insert(normalize::path(&offer.item).into_owned());
        }
    }

    let mut wiki: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for store in listed {
        let items: BTreeSet<String> = store
            .offers
            .iter()
            .filter_map(|printed| index.get(names::quantity(printed).1))
            .map(str::to_string)
            .collect();
        if !items.is_empty() {
            wiki.entry(person(store)).or_default().extend(items);
        }
    }
    let wiki: Vec<(&str, BTreeSet<String>)> = wiki
        .iter()
        .map(|(page, items)| (page.as_str(), items.clone()))
        .collect();

    let mut out = BTreeMap::new();
    for (key, items) in &sold {
        let best = wiki
            .iter()
            .map(|(name, theirs)| (*name, items.intersection(theirs).count()))
            .filter(|(_, shared)| *shared >= SURE_SHARED)
            .max_by_key(|(name, shared)| (*shared, std::cmp::Reverse(*name)));
        let Some((name, shared)) = best else { continue };
        let theirs = wiki
            .iter()
            .find(|(n, _)| *n == name)
            .expect("the match")
            .1
            .len();
        if shared as f64 / items.len().min(theirs) as f64 >= SURE_SHARE {
            out.insert(key.to_string(), name.to_string());
        }
    }
    out
}

/// What one offer asks for, in the one currency an offer can carry.
struct Asked {
    cost: Option<i64>,
    cost_max: Option<i64>,
    currency: Option<String>,
    pays: Option<String>,
    /// Every item the offer asks for, where it asks for more than one.
    also: Vec<graph::Cost>,
    floating: bool,
}

impl Asked {
    /// An ask in a currency rather than in items.
    fn paid(cost: i64, currency: &str, floating: bool, cost_max: Option<i64>) -> Self {
        Self {
            cost: Some(cost),
            cost_max,
            currency: Some(currency.to_string()),
            pays: None,
            also: Vec::new(),
            floating,
        }
    }
}

/// The price of an offer: items first, then standing, then platinum. A stall that asks for
/// several items at once — five common tags and five rare ones — names the first as the cost
/// and keeps the whole ask beside it, since no single number can stand for it.
fn asked(graph: &Graph, offer: &crate::offers::Offer) -> Asked {
    if let [price, rest @ ..] = offer.price.as_slice() {
        let pays = normalize::path(&price.item).into_owned();
        let also = match rest.is_empty() {
            true => Vec::new(),
            false => offer
                .price
                .iter()
                .map(|price| graph::Cost {
                    item: normalize::path(&price.item).into_owned(),
                    count: price.count,
                })
                .collect(),
        };
        return Asked {
            cost: Some(price.count),
            cost_max: None,
            currency: printed(graph, &pays),
            pays: Some(pays),
            also,
            floating: false,
        };
    }
    if let Some(standing) = offer.standing {
        return Asked::paid(standing, STANDING, false, None);
    }
    if let Some([low, high]) = offer.platinum {
        return Asked::paid(low, PLATINUM, high != low, (high != low).then_some(high));
    }
    Asked {
        cost: None,
        cost_max: None,
        currency: None,
        pays: None,
        also: Vec::new(),
        floating: offer.credits.is_some_and(|[low, high]| low != high),
    }
}

/// What the catalog calls the item a price is counted in.
fn printed(graph: &Graph, item: &str) -> Option<String> {
    match graph.get(item) {
        Some(Node::Item(item)) => Some(item.names.en.value.clone()),
        _ => None,
    }
}

/// A key per manifest, short where the last segment is enough and qualified where two
/// manifests end the same way. One person often keeps several manifests — Acrithis has one
/// per menu she offers — so a manifest named after another in the same directory is filed
/// under that one.
fn keys(stalls: &[Stall]) -> BTreeMap<&str, String> {
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    for stall in stalls {
        *taken.entry(short_key(&stall.manifest)).or_default() += 1;
    }
    stalls
        .iter()
        .map(|stall| {
            let keeper = keeper_of(stall, stalls);
            let short = short_key(keeper);
            let key = match season(keeper) {
                Some(_) => NIGHTWAVE.0.to_string(),
                None if taken[&short] > 1 => qualified_key(keeper),
                None => short,
            };
            (stall.manifest.as_str(), key)
        })
        .collect()
}

/// The manifest that names the person a stall belongs to: its own, or the shorter one beside
/// it whose name this one extends.
fn keeper_of<'a>(stall: &'a Stall, stalls: &'a [Stall]) -> &'a str {
    let (folder, name) = split(&stall.manifest);
    stalls
        .iter()
        .map(|other| other.manifest.as_str())
        .filter(|other| {
            let (their_folder, their_name) = split(other);
            their_folder == folder && their_name.len() < name.len() && name.starts_with(their_name)
        })
        .min_by_key(|other| split(other).1.len())
        .unwrap_or(&stall.manifest)
}

/// A manifest's directory and its own name, without the word every manifest ends with.
fn split(manifest: &str) -> (&str, &str) {
    let (folder, tail) = manifest.rsplit_once('/').unwrap_or(("", manifest));
    let name = tail
        .strip_suffix("VendorManifest")
        .or_else(|| tail.strip_suffix("Manifest"))
        .unwrap_or(tail);
    (folder, name)
}

/// The manifest's own name, without the word every manifest ends with.
fn short_key(manifest: &str) -> String {
    dashed(split(manifest).1)
}

/// A key from the name the game writes in one word: `TeshinHardMode` reads as
/// `teshin-hard-mode`.
fn dashed(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (at, letter) in name.char_indices() {
        if letter.is_uppercase() && at > 0 && !out.ends_with('-') {
            out.push('-');
        }
        out.extend(letter.to_lowercase());
    }
    slug(&out)
}

/// The manifest's name behind the directory it sits in, for the few that collide.
fn qualified_key(manifest: &str) -> String {
    let mut parts = manifest.rsplit('/');
    let tail = short_key(manifest);
    parts.next();
    match parts.next() {
        Some(parent) => format!("{}-{tail}", dashed(parent)),
        None => tail,
    }
}

/// The season a Nightwave manifest belongs to, where it is one.
fn season(manifest: &str) -> Option<u32> {
    let key = short_key(manifest);
    if !key.starts_with(SEASON) {
        return None;
    }
    let number: String = key.chars().filter(char::is_ascii_digit).collect();
    Some(number.parse().unwrap_or(0))
}

/// The season Nightwave is running now: the highest one the cache holds.
fn current_season(stalls: &[Stall]) -> Option<u32> {
    stalls
        .iter()
        .filter_map(|stall| season(&stall.manifest))
        .max()
}

/// A key read back as a name, for a vendor the game never names.
fn title(key: &str) -> String {
    if key == NIGHTWAVE.0 {
        return NIGHTWAVE.1.to_string();
    }
    key.split('-')
        .map(|word| {
            let mut letters = word.chars();
            match letters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The catalog item a currency is, where it is one: standing and platinum are not.
fn paid_in(index: &Index, currency: Option<&str>) -> Option<String> {
    currency
        .and_then(|currency| index.get(currency))
        .map(str::to_string)
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
                credits: it.credits,
                times: it.times,
                always: it.always,
                gone: it.gone,
                ..offer()
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
fn curated_of(graph: &mut Graph, stalls: &[Stall], index: &Index, curated: &Curation) -> Linked {
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

    let keys = keys(stalls);
    let listed: BTreeSet<(&str, String)> = stalls
        .iter()
        .flat_map(|stall| {
            let key = &keys[stall.manifest.as_str()];
            stall
                .offer
                .iter()
                .map(move |offer| (key.as_str(), normalize::path(&offer.item).into_owned()))
        })
        .collect();
    for sale in &curated.offer {
        let Some(item) = index.get(&sale.item) else {
            orphans::note(&mut out.unresolved, &sale.item, || vendor_id(&sale.vendor));
            continue;
        };
        if listed.contains(&(sale.vendor.as_str(), item.to_string())) {
            continue;
        }
        let from = vendor_id(&sale.vendor);
        let Some(Node::Vendor(vendor)) = graph.get(&from) else {
            orphans::note(&mut out.unresolved, &sale.item, || from.clone());
            continue;
        };
        let currency = match sale.currency.is_empty() {
            true => vendor.currency.clone().unwrap_or_default(),
            false => sale.currency.clone(),
        };
        out.offers += 1;
        // Credits are their own column: a counter that charges them charges them beside
        // whatever else it takes, and the market charges nothing else.
        let priced = match currency == CREDITS {
            true => Offer {
                credits: Some(sale.cost),
                ..offer()
            },
            false => Offer {
                cost: Some(sale.cost),
                pays: paid_in(index, Some(&currency)),
                currency: Some(currency),
                ..offer()
            },
        };
        graph.link(Edge {
            from: from.clone(),
            to: item.to_string(),
            rel: Rel::Sells(priced),
        });
    }
    out
}

/// A plain offer: one copy, always on the counter, its price still to be said.
fn offer() -> Offer {
    Offer {
        cost: None,
        cost_max: None,
        currency: None,
        pays: None,
        also: Vec::new(),
        store: None,
        credits: None,
        limit: None,
        floating: false,
        count: 1,
        rank: None,
        timer: None,
        times: 0,
        always: true,
        gone: false,
    }
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

/// Incarnon adapters by the leaf of their path, so a package can find the adapter it wraps.
fn adapters(graph: &Graph) -> BTreeMap<String, String> {
    graph
        .items()
        .filter(|item| item.unique_name.contains(ADAPTERS))
        .filter_map(|item| {
            let leaf = item.unique_name.rsplit_once('/')?.1;
            Some((leaf.to_string(), item.unique_name.clone()))
        })
        .collect()
}

/// The item a package hands over, where the package is named after it.
fn packaged(path: &str, adapters: &BTreeMap<String, String>) -> Option<String> {
    let (_, leaf) = path.rsplit_once('/')?;
    let (kind, made) = PACKAGED;
    let stem = leaf.strip_suffix(kind)?;
    adapters.get(&format!("{stem}{made}")).cloned()
}

/// What an offer is, when what it is is not an item.
fn not_an_item(path: &str) -> Option<&'static str> {
    NOT_AN_ITEM
        .iter()
        .find(|(part, _)| path.contains(part))
        .map(|(_, what)| *what)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offers::{self, Price};
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

    fn stall(manifest: &str, floating: bool, offer: Vec<offers::Offer>) -> Stall {
        Stall {
            manifest: manifest.to_string(),
            floating,
            offer,
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
    fn an_offer_names_what_it_is_paid_in_and_who_keeps_the_counter() {
        let mut graph = catalog(&[
            ("/Lotus/Types/Items/MiscItems/SteelEssence", "Steel Essence"),
            ("/Lotus/Weapons/Syam", "Syam"),
            ("/Lotus/Weapons/Hok", "Hok Special"),
        ]);
        let stalls = [
            stall(
                "/Lotus/Types/Game/VendorManifests/Hubs/TeshinHardModeVendorManifest",
                false,
                vec![offers::Offer {
                    item: "/Lotus/StoreItems/Weapons/Syam".to_string(),
                    price: vec![Price {
                        item: "/Lotus/Types/Items/MiscItems/SteelEssence".to_string(),
                        count: 15,
                    }],
                    hours: Some([24, 24]),
                    limit: Some(1),
                    always: true,
                    ..offers::Offer::default()
                }],
            ),
            stall(
                "/Lotus/Syndicates/Ostron/CetusManifest",
                false,
                vec![offers::Offer {
                    item: "/Lotus/StoreItems/Weapons/Hok".to_string(),
                    standing: Some(1000),
                    rank: Some(2),
                    store: Some("Weaponsmith".to_string()),
                    ..offers::Offer::default()
                }],
            ),
        ];

        stalls_of(
            &mut graph,
            &stalls,
            &Curation::default().terms(&BTreeMap::new()),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        let teshin = sold(&graph, "teshin-hard-mode");
        assert_eq!(teshin.cost, Some(15));
        assert_eq!(
            teshin.pays.as_deref(),
            Some("/Lotus/Types/Items/MiscItems/SteelEssence")
        );
        assert_eq!(teshin.currency.as_deref(), Some("Steel Essence"));
        assert_eq!((teshin.timer, teshin.limit), (Some(86_400), Some(1)));
        assert!(teshin.always && !teshin.floating);

        let hok = sold(&graph, "cetus-weaponsmith");
        assert_eq!(
            (hok.cost, hok.currency.as_deref()),
            (Some(1000), Some("Standing"))
        );
        assert_eq!(
            (hok.rank, hok.store.as_deref()),
            (Some(2), Some("Weaponsmith"))
        );
    }

    #[test]
    fn a_floating_stall_says_so_instead_of_naming_a_price() {
        let mut graph = catalog(&[("/Lotus/Types/Items/MiscItems/Ferrite", "Ferrite")]);
        let stalls = [stall(
            "/Lotus/Types/Game/VendorManifests/Duviri/AcrithisVendorManifest",
            true,
            vec![offers::Offer {
                item: "/Lotus/StoreItems/Types/Items/MiscItems/Ferrite".to_string(),
                hours: Some([1, 3]),
                ..offers::Offer::default()
            }],
        )];

        stalls_of(
            &mut graph,
            &stalls,
            &Curation::default().terms(&BTreeMap::new()),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        let offer = sold(&graph, "acrithis");
        assert!(offer.floating);
        assert_eq!((offer.cost, offer.currency), (None, None));
    }

    #[test]
    fn a_price_of_several_items_is_kept_whole() {
        let mut graph = catalog(&[
            ("/Lotus/Types/Items/Tag/Common", "Common Tag"),
            ("/Lotus/Types/Items/Tag/Rare", "Rare Tag"),
            ("/Lotus/Upgrades/Plushy", "Plushy"),
        ]);
        let stalls = [stall(
            "/Lotus/Types/Game/VendorManifests/Deimos/ConservationRewardsManifest",
            false,
            vec![offers::Offer {
                item: "/Lotus/StoreItems/Upgrades/Plushy".to_string(),
                price: vec![
                    Price {
                        item: "/Lotus/Types/Items/Tag/Common".to_string(),
                        count: 5,
                    },
                    Price {
                        item: "/Lotus/Types/Items/Tag/Rare".to_string(),
                        count: 5,
                    },
                ],
                ..offers::Offer::default()
            }],
        )];

        stalls_of(
            &mut graph,
            &stalls,
            &Curation::default().terms(&BTreeMap::new()),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        let offer = sold(&graph, "necralisk-conservation-rewards");
        assert_eq!(
            (offer.cost, offer.pays.as_deref()),
            (Some(5), Some("/Lotus/Types/Items/Tag/Common"))
        );
        assert_eq!(
            offer.also,
            [
                graph::Cost {
                    item: "/Lotus/Types/Items/Tag/Common".to_string(),
                    count: 5,
                },
                graph::Cost {
                    item: "/Lotus/Types/Items/Tag/Rare".to_string(),
                    count: 5,
                },
            ]
        );
    }

    #[test]
    fn a_closed_season_only_adds_what_nobody_sells_any_more() {
        let mut graph = catalog(&[
            ("/Lotus/Weapons/Nora", "Nora Special"),
            ("/Lotus/Weapons/Retired", "Retired Glyph"),
        ]);
        let season = |manifest: &str, items: &[&str]| {
            stall(
                manifest,
                false,
                items
                    .iter()
                    .map(|item| offers::Offer {
                        item: format!("/Lotus/StoreItems{item}"),
                        ..offers::Offer::default()
                    })
                    .collect(),
            )
        };
        let stalls = [
            season(
                "/Lotus/Types/Game/VendorManifests/RadioLegionIntermission15VendorManifest",
                &["/Weapons/Nora", "/Weapons/Retired"],
            ),
            season(
                "/Lotus/Types/Game/VendorManifests/RadioLegionIntermission16VendorManifest",
                &["/Weapons/Nora"],
            ),
        ];

        stalls_of(
            &mut graph,
            &stalls,
            &Curation::default().terms(&BTreeMap::new()),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        let sold: BTreeMap<&str, bool> = graph
            .from(&vendor_id("nightwave"))
            .into_iter()
            .filter_map(|edge| match &edge.rel {
                Rel::Sells(offer) => Some((edge.to.as_str(), offer.gone)),
                _ => None,
            })
            .collect();
        assert!(!sold["/Lotus/Weapons/Nora"]);
        assert!(sold["/Lotus/Weapons/Retired"]);
        assert!(matches!(
            graph.get(&vendor_id("nightwave")),
            Some(Node::Vendor(vendor)) if vendor.name == "Nightwave"
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

    #[test]
    fn a_package_sells_the_adapter_it_wraps_and_a_job_sells_nothing() {
        let mut graph = catalog(&[(
            "/Lotus/Types/Items/MiscItems/IncarnonAdapters/Primary/BoarIncarnonUnlocker",
            "Boar Incarnon Genesis",
        )]);
        let stalls = [stall(
            "/Lotus/Types/Game/VendorManifests/Zariman/ZarimanWeaponsmithIncarnonShopManifest",
            false,
            vec![
                offers::Offer {
                    item: "/Lotus/Types/StoreItems/Packages/IncarnonPackages/BoarIncarnonBundle"
                        .to_string(),
                    platinum: Some([120, 120]),
                    ..offers::Offer::default()
                },
                offers::Offer {
                    item: "/Lotus/Types/StoreItems/Packages/Tasks/Deimos/Daughter/DaughterTaskA"
                        .to_string(),
                    ..offers::Offer::default()
                },
            ],
        )];

        let linked = stalls_of(
            &mut graph,
            &stalls,
            &Curation::default().terms(&BTreeMap::new()),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        let sells: Vec<&str> = graph
            .from(&vendor_id("zariman-weaponsmith-incarnon-shop"))
            .into_iter()
            .filter(|edge| matches!(edge.rel, Rel::Sells(_)))
            .map(|edge| edge.to.as_str())
            .collect();
        assert_eq!(
            sells,
            ["/Lotus/Types/Items/MiscItems/IncarnonAdapters/Primary/BoarIncarnonUnlocker"]
        );
        assert!(linked.unresolved.is_empty());
        assert_eq!(linked.not_items["a job the family's stall arranges"], 1);
    }

    #[test]
    fn two_counters_named_the_same_are_one_vendor() {
        let mut graph = catalog(&[
            ("/Lotus/Types/Items/MiscItems/Fish", "Fish"),
            ("/Lotus/Types/Items/MiscItems/Bait", "Bait"),
        ]);
        let stalls = [
            stall(
                "/Lotus/Types/Game/VendorManifests/Solaris/FortunaFishmongerVendorManifest",
                false,
                vec![offers::Offer {
                    item: "/Lotus/StoreItems/Types/Items/MiscItems/Fish".to_string(),
                    ..offers::Offer::default()
                }],
            ),
            stall(
                "/Lotus/Types/Game/VendorManifests/Solaris/TheBusinessVendorManifest",
                false,
                vec![offers::Offer {
                    item: "/Lotus/StoreItems/Types/Items/MiscItems/Bait".to_string(),
                    ..offers::Offer::default()
                }],
            ),
        ];
        let mut curated = Curation::default();
        curated.set_term("vendor", "fortuna-fishmonger", "Бизнес");
        curated.set_term("vendor", "the-business", "Бизнес");

        let linked = stalls_of(
            &mut graph,
            &stalls,
            &curated.terms(&BTreeMap::new()),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        assert_eq!(linked.vendors, 1);
        let sells = graph
            .from(&vendor_id("fortuna-fishmonger"))
            .into_iter()
            .filter(|edge| matches!(edge.rel, Rel::Sells(_)))
            .count();
        assert_eq!(sells, 2);
    }

    #[test]
    fn a_vendor_key_survives_punctuation() {
        assert_eq!(slug("Cephalon Simaris"), "cephalon-simaris");
        assert_eq!(slug("Kahl's Garrison"), "kahl-s-garrison");
        assert_eq!(slug("The Perrin Sequence"), "the-perrin-sequence");
    }
}
