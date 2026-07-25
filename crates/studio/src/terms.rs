use std::collections::BTreeMap;

use consensus::Source;
use graph::Node;

use crate::state::Snapshot;

/// Everything the catalog ships that carries a name, and therefore needs a Russian one. Items
/// are named by `[[name]]`, everything else by `[[term]]`, because only an item has a path of
/// its own to key a name by.
pub const TARGETS: [&str; 13] = [
    "item",
    "place",
    "region",
    "planet",
    "vendor",
    "lab",
    "mission",
    "faction",
    "node_type",
    "settlement",
    "giver",
    "activity",
    "kind",
];

/// One thing to translate.
pub struct Row {
    pub key: String,
    pub en: String,
    pub ru: Option<String>,
    /// Where the Russian came from, for items — the other kinds only ever have a hand or a
    /// reference table behind them.
    pub from: Option<Source>,
    /// What it is, so a name can be judged without opening it: the planet, the class, the
    /// vendor's currency.
    pub note: String,
}

impl Row {
    fn new(key: &str, en: &str, ru: Option<&str>, note: String) -> Self {
        Self {
            key: key.to_string(),
            en: en.to_string(),
            ru: ru.map(str::to_string),
            from: None,
            note,
        }
    }
}

/// Everything of one kind that needs a Russian name, in a stable order.
pub fn rows(snap: &Snapshot, target: &str) -> Vec<Row> {
    match target {
        "item" => items(snap),
        "place" => places(snap),
        "region" => regions(snap),
        "planet" => planets(snap),
        "vendor" => vendors(snap),
        "lab" => labs(snap),
        "mission" | "faction" | "node_type" => labels(snap, target),
        "settlement" | "giver" | "activity" => bounties(snap, target),
        "kind" => kinds(snap),
        _ => Vec::new(),
    }
}

/// How many things of one kind there are, and how many still have no Russian name. Items are
/// counted without building the list: there are eighteen thousand of them and this runs for
/// every kind at once.
pub fn tally(snap: &Snapshot, target: &str) -> (usize, usize) {
    if target == "item" {
        let mut total = 0;
        let mut missing = 0;
        for item in snap.graph.items() {
            total += 1;
            missing += usize::from(item.names.ru.is_none());
        }
        return (total, missing);
    }
    let rows = rows(snap, target);
    let missing = rows.iter().filter(|r| r.ru.is_none()).count();
    (rows.len(), missing)
}

fn items(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .items()
        .map(|i| Row {
            key: i.unique_name.clone(),
            en: i.names.en.value.clone(),
            ru: i.names.ru.as_ref().map(|r| r.value.clone()),
            from: i.names.ru.as_ref().map(|r| r.winner),
            note: snap.taxonomy.label(&i.kind.value, "ru").to_string(),
        })
        .collect()
}

fn places(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Place(p) => Some(Row::new(
                &p.name,
                &p.name,
                p.name_ru.as_deref(),
                crate::words::place(p.kind).to_string(),
            )),
            _ => None,
        })
        .collect()
}

fn regions(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Region(r) => Some(Row::new(
                &r.node,
                &r.name,
                r.name_ru.as_deref(),
                r.planet_ru.clone().unwrap_or_else(|| r.planet.clone()),
            )),
            _ => None,
        })
        .collect()
}

fn planets(snap: &Snapshot) -> Vec<Row> {
    let mut seen: BTreeMap<&str, Option<&str>> = BTreeMap::new();
    for node in snap.graph.nodes() {
        if let Node::Region(r) = node {
            let slot = seen.entry(r.planet.as_str()).or_default();
            if slot.is_none() {
                *slot = r.planet_ru.as_deref();
            }
        }
    }
    seen.into_iter()
        .map(|(planet, ru)| Row::new(planet, planet, ru, String::new()))
        .collect()
}

fn vendors(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Vendor(v) => Some(Row::new(
                &v.key,
                &v.name,
                v.name_ru.as_deref(),
                match &v.currency {
                    Some(currency) => {
                        format!("{} · {currency}", crate::words::vendor(v.kind.as_deref()))
                    }
                    None => crate::words::vendor(v.kind.as_deref()).to_string(),
                },
            )),
            _ => None,
        })
        .collect()
}

fn labs(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Lab(l) => Some(Row::new(&l.key, &l.name, l.name_ru.as_deref(), l.faction.clone())),
            _ => None,
        })
        .collect()
}

/// The star chart's own enums, as the nodes carry them. An index the reference table does not
/// name has no English label either, so there is nothing to translate and it is left out.
fn labels(snap: &Snapshot, target: &str) -> Vec<Row> {
    let mut seen: BTreeMap<&str, (Option<&str>, usize)> = BTreeMap::new();
    for node in snap.graph.nodes() {
        let Node::Region(r) = node else { continue };
        let label = match target {
            "mission" => &r.mission_label,
            "faction" => &r.faction_label,
            _ => &r.type_label,
        };
        let Some(en) = label.en.as_deref() else {
            continue;
        };
        let slot = seen.entry(en).or_insert((None, 0));
        slot.1 += 1;
        if slot.0.is_none() {
            slot.0 = label.ru.as_deref();
        }
    }
    seen.into_iter()
        .map(|(en, (ru, count))| Row::new(en, en, ru, format!("{count} узлов")))
        .collect()
}

/// The words the bounty tables are named by: where the bounty is given, by whom, and what the
/// printed table calls the activity.
fn bounties(snap: &Snapshot, target: &str) -> Vec<Row> {
    let mut seen: BTreeMap<String, Option<String>> = BTreeMap::new();
    for node in snap.graph.nodes() {
        let Node::Place(p) = node else { continue };
        let Some(b) = &p.bounty else { continue };
        let pair = match target {
            "settlement" => (Some(b.settlement.clone()), b.settlement_ru.clone()),
            "giver" => (b.giver.clone(), b.giver_ru.clone()),
            _ => (Some(b.activity.clone()), b.activity_ru.clone()),
        };
        let (Some(en), ru) = pair else { continue };
        let slot = seen.entry(en).or_default();
        if slot.is_none() {
            *slot = ru;
        }
    }
    seen.into_iter()
        .map(|(en, ru)| Row::new(&en, &en, ru.as_deref(), String::new()))
        .collect()
}

/// Our own tree: the words the app groups the catalog by.
fn kinds(snap: &Snapshot) -> Vec<Row> {
    let mut out = Vec::new();
    for class in snap.taxonomy.classes() {
        out.push(Row::new(&class.slug, &class.en, Some(&class.ru), "класс".into()));
        for leaf in &class.kind {
            out.push(Row::new(&leaf.slug, &leaf.en, Some(&leaf.ru), class.ru.clone()));
        }
    }
    out
}

/// The graph node behind a row, where there is one, so a name leads to the page that holds
/// everything else about it. A planet, a mission type or a word from a bounty table is not a
/// node — it is only ever a label.
pub fn node_id(target: &str, key: &str) -> Option<String> {
    match target {
        "item" => Some(key.to_string()),
        "place" => Some(graph::place_id(key)),
        "region" => Some(graph::region_id(key)),
        "vendor" => Some(graph::vendor_id(key)),
        "lab" => Some(graph::lab_id(key)),
        _ => None,
    }
}

/// Everything that carries a name and has none in Russian, counted in one pass over the graph
/// for the sidebar. The tree's own labels are declared with both languages, so they never
/// count as missing.
pub fn pending(snap: &Snapshot) -> usize {
    snap.graph
        .nodes()
        .filter(|n| {
            matches!(
                n,
                Node::Item(_) | Node::Place(_) | Node::Region(_) | Node::Vendor(_) | Node::Lab(_)
            ) && n.label_ru().is_none()
        })
        .count()
}
