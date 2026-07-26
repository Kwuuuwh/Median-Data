use graph::{DropInfo, Edge, Graph, Node, Place, PlaceKind, Rel, place_id};
use sources::drops::Drop;

use crate::bounties::Settlements;
use crate::names::{self, Index};
use crate::orphans::{self, Missing};

/// What could not be attached, for the coverage report.
#[derive(Default)]
pub struct Missed {
    /// Printed names matching no item in the catalog, with where and how often.
    pub unknown: Missing,
    /// Rows rewarding an amount (endo, credits) rather than an item.
    pub not_an_item: usize,
    /// Rows whose printed name matches several items.
    pub ambiguous: usize,
}

/// Place nodes with an edge per drop line. Rows naming an unknown item are counted.
pub fn link(
    graph: &mut Graph,
    rows: &[Drop],
    settlements: &Settlements,
    index: &Index,
    terms: &crate::curation::Terms,
) -> Missed {
    let mut missed = Missed::default();

    for row in rows {
        let (count, printed) = names::quantity(&row.item);
        if names::is_amount(printed) {
            missed.not_an_item += 1;
            continue;
        }
        let Some(item) = index.get(printed).map(str::to_string) else {
            orphans::note(&mut missed.unknown, printed, || row.place.clone());
            continue;
        };
        if index.is_ambiguous(printed) {
            missed.ambiguous += 1;
        }

        let place = place_id(&row.place);
        graph.insert(Node::Place(Place {
            name: row.place.clone(),
            name_ru: terms.get("place", &row.place).map(str::to_string),
            kind: kind_of(&row.section),
            bounty: settlements
                .read(&row.section, &row.place)
                .map(|b| localize(b, terms)),
        }));
        graph.link(Edge {
            from: place,
            to: item,
            rel: Rel::Drops(DropInfo {
                rarity: row.rarity.clone(),
                chance: row.chance,
                rotation: row.rotation.clone(),
                stage: row.stage.clone(),
                table_chance: row.table_chance,
                count,
            }),
        });
    }
    missed
}

/// A bounty with whatever Russian was written for its settlement, its giver and the activity
/// its table is named after.
fn localize(mut bounty: graph::Bounty, terms: &crate::curation::Terms) -> graph::Bounty {
    bounty.settlement_ru = terms.or("settlement", &bounty.settlement, bounty.settlement_ru);
    if let Some(giver) = &bounty.giver {
        bounty.giver_ru = terms.or("giver", giver, bounty.giver_ru.clone());
    }
    bounty.activity_ru = terms.or("activity", &bounty.activity, bounty.activity_ru);
    bounty
}

/// The kind of place a drop-table section describes.
fn kind_of(section: &str) -> PlaceKind {
    match section {
        "keyRewards" => PlaceKind::Key,
        "sortieRewards" => PlaceKind::Sortie,
        "transientRewards" => PlaceKind::Transient,
        s if s.ends_with("ByAvatar") || s.ends_with("ByDrop") => PlaceKind::Enemy,
        s if s.ends_with("Rewards") && s != "missionRewards" => PlaceKind::Bounty,
        _ => PlaceKind::Node,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_map_to_place_kinds() {
        assert_eq!(kind_of("missionRewards"), PlaceKind::Node);
        assert_eq!(kind_of("keyRewards"), PlaceKind::Key);
        assert_eq!(kind_of("cetusRewards"), PlaceKind::Bounty);
        assert_eq!(kind_of("modByAvatar"), PlaceKind::Enemy);
        assert_eq!(kind_of("resourceByDrop"), PlaceKind::Enemy);
    }
}
