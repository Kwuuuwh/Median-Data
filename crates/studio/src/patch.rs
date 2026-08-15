use consensus::{Resolved, Source, Status};
use graph::{Kind, Node};

use crate::state::Snapshot;

/// Show a decision in the snapshot that is already on screen. Only what the decision plainly
/// says is changed; everything derived from it — an edge a mapping creates, a check a name
/// clears — waits for the next full assembly, which is what `stale` announces.
fn touched(snap: &mut Snapshot) {
    snap.stale = true;
}

/// A printed name tied to an item: the row leaves the mapping list, and the decision joins
/// the ones on record.
pub fn mapped(snap: &mut Snapshot, source: &str, key: &str, item: &str) {
    snap.unresolved
        .retain(|u| u.source != source || u.key != key);
    let links = &mut snap.decided.links;
    links.retain(|(s, k, _)| s != source || k != key);
    links.push((source.into(), key.into(), item.into()));
    links.sort();
    touched(snap);
}

pub fn unmapped(snap: &mut Snapshot, source: &str, key: &str) {
    snap.decided
        .links
        .retain(|(s, k, _)| s != source || k != key);
    touched(snap);
}

/// A printed name declared to be no item at all: it leaves the mapping list, and the check
/// that reported it stops asking.
pub fn dismissed(snap: &mut Snapshot, source: &str, key: &str, note: &str) {
    snap.unresolved
        .retain(|u| u.source != source || u.key != key);
    let rows = &mut snap.decided.dismissed;
    rows.retain(|(s, k, _)| s != source || k != key);
    rows.push((source.into(), key.into(), note.trim().into()));
    rows.sort();
    if source == crate::mapping::DROPS {
        accepted(snap, funnel::DROP_NOT_IN_CATALOG, key);
    }
    touched(snap);
}

/// A printed name asked about again.
pub fn undismissed(snap: &mut Snapshot, source: &str, key: &str) {
    snap.decided
        .dismissed
        .retain(|(s, k, _)| s != source || k != key);
    if source == crate::mapping::DROPS {
        unaccepted(snap, funnel::DROP_NOT_IN_CATALOG, key);
    }
    touched(snap);
}

/// A Russian name written for an item. An empty name takes the decision back, which leaves
/// whatever the sources said standing until the next assembly.
pub fn named(snap: &mut Snapshot, item: &str, ru: &str) {
    let names = &mut snap.decided.names;
    names.retain(|(i, _)| i != item);
    if !ru.trim().is_empty() {
        names.push((item.into(), ru.trim().into()));
        names.sort();
        if let Some(Node::Item(i)) = snap.graph.get_mut(item) {
            i.names.ru = Some(by_hand(ru.trim().to_string()));
        }
        snap.report
            .findings
            .retain(|f| f.rule != "russian-name-missing" || f.entity != item);
    }
    touched(snap);
}

/// A value kept where sources disagreed. The disagreement goes off the list, and the value
/// itself lands where the screens read it.
pub fn picked(snap: &mut Snapshot, item: &str, prop: &str, value: &str) {
    let picks = &mut snap.decided.picks;
    picks.retain(|(i, p, _)| i != item || p != prop);
    if !value.trim().is_empty() {
        picks.push((item.into(), prop.into(), value.trim().into()));
        picks.sort();
        apply(snap, item, prop, value.trim());
        snap.conflicts
            .retain(|c| c.entity != item || c.prop != prop);
    }
    touched(snap);
}

fn apply(snap: &mut Snapshot, item: &str, prop: &str, value: &str) {
    let Some(Node::Item(i)) = snap.graph.get_mut(item) else {
        return;
    };
    match prop {
        "name_ru" => i.names.ru = Some(by_hand(value.to_string())),
        "name_en" => i.names.en = by_hand(value.to_string()),
        "tradable" => i.tradable = Some(by_hand(value == "true")),
        "prime" => i.prime = by_hand(value == "true"),
        "kind" => i.kind = by_hand(Kind::new(value)),
        _ => {}
    }
}

/// A Russian word written for something that is not an item.
pub fn termed(snap: &mut Snapshot, kind: &str, key: &str, ru: &str) {
    let terms = &mut snap.decided.terms;
    terms.retain(|(k, id, _)| k != kind || id != key);
    let word = ru.trim();
    if !word.is_empty() {
        terms.push((kind.into(), key.into(), word.into()));
        terms.sort();
        write_term(snap, kind, key, word);
    }
    touched(snap);
}

/// Record, or take back, the verdict that a name stays as the game writes it.
pub fn verbatim(snap: &mut Snapshot, kind: &str, key: &str, note: &str, kept: bool) {
    let judged = &mut snap.decided.verbatim;
    judged.retain(|(k, id, _)| k != kind || id != key);
    if kept {
        judged.push((kind.into(), key.into(), note.trim().into()));
        judged.sort();
    }
    touched(snap);
}

fn write_term(snap: &mut Snapshot, kind: &str, key: &str, ru: &str) {
    let word = || Some(ru.to_string());
    match kind {
        "kind" => snap.taxonomy.relabel(key, ru),
        "place" => {
            if let Some(Node::Place(p)) = snap.graph.get_mut(&graph::place_id(key)) {
                p.name_ru = word();
            }
        }
        "enemy" => {
            if let Some(Node::Enemy(e)) = snap.graph.get_mut(&graph::enemy_id(key)) {
                e.name_ru = word();
            }
        }
        "location" => {
            if let Some(Node::Location(l)) = snap.graph.get_mut(&graph::location_id(key)) {
                l.name_ru = word();
            }
        }
        "region" => {
            if let Some(Node::Region(r)) = snap.graph.get_mut(&graph::region_id(key)) {
                r.name_ru = word();
            }
        }
        "vendor" => {
            if let Some(Node::Vendor(v)) = snap.graph.get_mut(&graph::vendor_id(key)) {
                v.name_ru = word();
            }
        }
        "lab" => {
            if let Some(Node::Lab(l)) = snap.graph.get_mut(&graph::lab_id(key)) {
                l.name_ru = word();
            }
        }
        // A mission type, a faction and a bounty's words are carried by every node that uses
        // them, so each one is written wherever it appears.
        other => spread(snap, other, key, ru),
    }
}

fn spread(snap: &mut Snapshot, kind: &str, key: &str, ru: &str) {
    for node in snap.graph.nodes_mut() {
        match node {
            Node::Region(r) => match kind {
                "mission" if r.mission_label.en.as_deref() == Some(key) => {
                    r.mission_label.ru = Some(ru.to_string())
                }
                "faction" if r.faction_label.en.as_deref() == Some(key) => {
                    r.faction_label.ru = Some(ru.to_string())
                }
                "node_type" if r.type_label.en.as_deref() == Some(key) => {
                    r.type_label.ru = Some(ru.to_string())
                }
                "tileset" if r.tileset.en.as_deref() == Some(key) => {
                    r.tileset.ru = Some(ru.to_string())
                }
                _ => {}
            },
            Node::Place(p) => {
                let Some(b) = &mut p.bounty else { continue };
                match kind {
                    "settlement" if b.settlement == key => b.settlement_ru = Some(ru.to_string()),
                    "giver" if b.giver.as_deref() == Some(key) => b.giver_ru = Some(ru.to_string()),
                    "activity" if b.activity == key => b.activity_ru = Some(ru.to_string()),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// A finding let through: it leaves the list and joins what was accepted, keeping its detail.
pub fn accepted(snap: &mut Snapshot, rule: &str, entity: &str) {
    let report = &mut snap.report;
    if let Some(at) = report
        .findings
        .iter()
        .position(|f| f.rule == rule && f.entity == entity)
    {
        let taken = report.findings.remove(at);
        report.accepted.push(taken);
    }
    touched(snap);
}

/// A finding asked about again.
pub fn unaccepted(snap: &mut Snapshot, rule: &str, entity: &str) {
    let report = &mut snap.report;
    if let Some(at) = report
        .accepted
        .iter()
        .position(|f| f.rule == rule && f.entity == entity)
    {
        let back = report.accepted.remove(at);
        report.findings.push(back);
    }
    touched(snap);
}

fn by_hand<T>(value: T) -> Resolved<T> {
    Resolved {
        value,
        status: Status::Single,
        winner: Source::Curated,
        sources: vec![Source::Curated],
    }
}
