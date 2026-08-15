use std::collections::BTreeMap;

use consensus::Source;
use graph::Node;

use crate::state::Snapshot;

/// Everything the catalog ships that carries a name, and therefore needs a Russian one. Items
/// are named by `[[name]]`, everything else by `[[term]]`, because only an item has a path of
/// its own to key a name by.
pub const TARGETS: [&str; 15] = [
    "item",
    "enemy",
    "place",
    "region",
    "location",
    "vendor",
    "lab",
    "mission",
    "faction",
    "node_type",
    "tileset",
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
    /// Judged to stay as the game writes it, so no Russian is owed. Carries why.
    pub verbatim: Option<String>,
}

impl Row {
    fn new(key: &str, en: &str, ru: Option<&str>, note: String) -> Self {
        Self {
            key: key.to_string(),
            en: en.to_string(),
            ru: ru.map(str::to_string),
            from: None,
            note,
            verbatim: None,
        }
    }
}

/// Everything of one kind that needs a Russian name, in a stable order, each carrying the
/// verdict that it stays English where one was written.
pub fn rows(snap: &Snapshot, target: &str) -> Vec<Row> {
    let mut rows = gather(snap, target);
    for row in &mut rows {
        // A hand decision replaces whatever the sources implied; where there is none, a
        // verdict the build worked out itself stands.
        if let Some((_, _, note)) = snap
            .decided
            .verbatim
            .iter()
            .find(|(kind, key, _)| kind == target && *key == row.key)
        {
            row.verbatim = Some(note.clone());
        }
    }
    rows
}

fn gather(snap: &Snapshot, target: &str) -> Vec<Row> {
    match target {
        "item" => items(snap),
        "enemy" => enemies(snap),
        "place" => places(snap),
        "region" => regions(snap),
        "location" => locations(snap),
        "vendor" => vendors(snap),
        "lab" => labs(snap),
        "mission" | "faction" | "node_type" | "tileset" => labels(snap, target),
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
        let judged: usize = snap
            .decided
            .verbatim
            .iter()
            .filter(|(kind, _, _)| kind == target)
            .count();
        for item in snap.graph.items() {
            total += 1;
            missing += usize::from(item.names.ru.is_none());
        }
        return (total, missing.saturating_sub(judged));
    }
    let rows = rows(snap, target);
    let missing = rows
        .iter()
        .filter(|r| r.ru.is_none() && r.verbatim.is_none())
        .count();
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
            verbatim: None,
        })
        .collect()
}

/// The places, each shown by what it actually is. A node's reward table is printed as one
/// string — `Ceres/Bode (Spy)` — so the node is put in the name column and the rest, which
/// the star chart already holds as data, reads as what the row is about.
fn places(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Place(p) => {
                let kind = crate::words::place(p.kind);
                let table = p.table.as_ref();
                let (en, note) = match &p.table {
                    Some(table) => {
                        let mut about = vec![table.location.clone(), table.label.clone()];
                        if table.extra {
                            about.push("доп. таблица".into());
                        }
                        if table.event {
                            about.push("событие".into());
                        }
                        (table.node.clone(), about.join(" · "))
                    }
                    None => (p.name.clone(), kind.to_string()),
                };
                let mut row = Row::new(&p.name, &en, p.name_ru.as_deref(), note);
                // A node's reward table is read as its node and its mission type, both of
                // which are named elsewhere, so the heading itself is nothing to translate.
                if table.is_some() {
                    row.verbatim = Some("читается по узлу".into());
                }
                Some(row)
            }
            _ => None,
        })
        .collect()
}

fn enemies(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Enemy(e) => Some(Row::new(
                &e.name,
                &e.name,
                e.name_ru.as_deref(),
                String::new(),
            )),
            _ => None,
        })
        .collect()
}

fn regions(snap: &Snapshot) -> Vec<Row> {
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Region(r) => {
                let mut row = Row::new(
                    &r.node,
                    &r.name,
                    r.name_ru.as_deref(),
                    snap.graph
                        .get(&graph::location_id(&r.location))
                        .and_then(|n| n.label_ru())
                        .unwrap_or(&r.location)
                        .to_string(),
                );
                // DE shipped the Russian manifest and wrote the same name in it, so this is
                // an answer, not a gap.
                if r.verbatim {
                    row.verbatim = Some("DE пишет так же".into());
                }
                Some(row)
            }
            _ => None,
        })
        .collect()
}

/// The groupings the star chart files its nodes under, with what each one is and how many
/// nodes sit in it. A grouping the reference table gives no kind is marked as such.
fn locations(snap: &Snapshot) -> Vec<Row> {
    let mut nodes: BTreeMap<&str, usize> = BTreeMap::new();
    for node in snap.graph.nodes() {
        if let Node::Region(r) = node {
            *nodes.entry(r.location.as_str()).or_default() += 1;
        }
    }
    snap.graph
        .nodes()
        .filter_map(|n| match n {
            Node::Location(l) => {
                let count = nodes.get(l.name.as_str()).copied().unwrap_or(0);
                let word = crate::words::plural(count, "узел", "узла", "узлов");
                let kind = match &l.kind {
                    Some(kind) => crate::words::location(kind),
                    None => "тип не указан",
                };
                Some(Row::new(
                    &l.name,
                    &l.name,
                    l.name_ru.as_deref(),
                    format!("{kind} · {count} {word}"),
                ))
            }
            _ => None,
        })
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
            Node::Lab(l) => Some(Row::new(
                &l.key,
                &l.name,
                l.name_ru.as_deref(),
                l.faction.clone(),
            )),
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
            "tileset" => &r.tileset,
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
        .map(|(en, (ru, count))| {
            let word = crate::words::plural(count, "узел", "узла", "узлов");
            Row::new(en, en, ru, format!("{count} {word}"))
        })
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
        out.push(Row::new(
            &class.slug,
            &class.en,
            Some(&class.ru),
            "класс".into(),
        ));
        for leaf in &class.kind {
            out.push(Row::new(
                &leaf.slug,
                &leaf.en,
                Some(&leaf.ru),
                class.ru.clone(),
            ));
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
        "enemy" => Some(graph::enemy_id(key)),
        "place" => Some(graph::place_id(key)),
        "location" => Some(graph::location_id(key)),
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
    let judged = snap.decided.verbatim.len();
    let missing = snap
        .graph
        .nodes()
        .filter(|n| {
            matches!(
                n,
                Node::Item(_)
                    | Node::Enemy(_)
                    | Node::Place(_)
                    | Node::Location(_)
                    | Node::Region(_)
                    | Node::Vendor(_)
                    | Node::Lab(_)
            ) && n.label_ru().is_none()
            // Answered already: a node DE writes the same way in both manifests, and a
            // reward table that reads as its node.
            && !matches!(n, Node::Region(r) if r.verbatim)
            && !matches!(n, Node::Place(p) if p.table.is_some())
        })
        .count();
    missing.saturating_sub(judged)
}
