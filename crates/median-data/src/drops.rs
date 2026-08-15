use std::collections::BTreeSet;

use graph::{
    DropInfo, Edge, Enemy, Graph, Levels, Node, Place, PlaceKind, Rel, enemy_id, place_id,
};
use sources::drops::Drop;

use crate::bounties::Settlements;
use crate::names::{self, Index};
use crate::orphans::{self, Missing};

/// One drop line reduced to what makes it distinct, so the same row printed in two sections
/// lands once.
type Line = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<Levels>,
    u64,
    Option<u64>,
    Option<i64>,
);

/// What could not be attached, for the coverage report.
#[derive(Default)]
pub struct Missed {
    /// Printed names matching no item in the catalog, with where and how often.
    pub unknown: Missing,
    /// Rows rewarding an amount (endo, credits) rather than an item.
    pub not_an_item: usize,
    /// Rows whose printed name matches several items.
    pub ambiguous: usize,
    /// Rows the tables print twice: the enemy sections are also printed transposed, by item.
    pub repeated: usize,
}

/// What the drop tables added to the graph.
#[derive(Default)]
pub struct Linked {
    pub places: usize,
    pub enemies: usize,
    pub missed: Missed,
}

/// Place and enemy nodes with an edge per drop line. Rows naming an unknown item are counted.
pub fn link(
    graph: &mut Graph,
    rows: &[Drop],
    settlements: &Settlements,
    index: &Index,
    terms: &crate::curation::Terms,
) -> Linked {
    let mut out = Linked::default();
    let mut seen: BTreeSet<Line> = BTreeSet::new();

    for row in rows {
        let (count, printed) = names::quantity(&row.item);
        if names::is_amount(printed) {
            out.missed.not_an_item += 1;
            continue;
        }
        let Some(item) = index.get(printed).map(str::to_string) else {
            orphans::note(&mut out.missed.unknown, printed, || {
                match by_enemy(&row.section) {
                    true => enemy_id(split_levels(&row.place).0),
                    false => place_id(&row.place),
                }
            });
            continue;
        };
        if index.is_ambiguous(printed) {
            out.missed.ambiguous += 1;
        }

        let (from, levels) = match by_enemy(&row.section) {
            true => {
                let (name, levels) = split_levels(&row.place);
                if graph.insert(Node::Enemy(Enemy {
                    name: name.to_string(),
                    name_ru: terms.get("enemy", name).map(str::to_string),
                })) {
                    out.enemies += 1;
                }
                (enemy_id(name), levels)
            }
            false => {
                let kind = kind_of(&row.section);
                if graph.insert(Node::Place(Place {
                    name: row.place.clone(),
                    name_ru: terms.get("place", &row.place).map(str::to_string),
                    kind,
                    bounty: settlements
                        .read(&row.section, &row.place)
                        .map(|b| localize(b, terms)),
                    table: match kind {
                        PlaceKind::Node => crate::rules::heading(&row.place),
                        _ => None,
                    },
                })) {
                    out.places += 1;
                }
                (place_id(&row.place), None)
            }
        };

        let line: Line = (
            from.clone(),
            item.clone(),
            row.rarity.clone(),
            row.rotation.clone(),
            row.stage.clone(),
            levels,
            row.chance.to_bits(),
            row.table_chance.map(f64::to_bits),
            count,
        );
        if !seen.insert(line) {
            out.missed.repeated += 1;
            continue;
        }

        graph.link(Edge {
            from,
            to: item,
            rel: Rel::Drops(DropInfo {
                rarity: row.rarity.clone(),
                chance: row.chance,
                rotation: row.rotation.clone(),
                stage: row.stage.clone(),
                table_chance: row.table_chance,
                levels,
                count,
            }),
        });
    }
    out
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

/// Whether a section names enemies rather than places. `ByDrop` is the same table transposed,
/// item first, so both discharge into the same enemy.
fn by_enemy(section: &str) -> bool {
    section.ends_with("ByAvatar") || section.ends_with("ByDrop")
}

/// Split `Name (Level 0 - 50)` into the enemy and the range its table is printed for.
fn split_levels(printed: &str) -> (&str, Option<Levels>) {
    let Some((name, rest)) = printed.rsplit_once("(Level") else {
        return (printed, None);
    };
    let Some((min, max)) = rest
        .trim_end()
        .strip_suffix(')')
        .and_then(|r| r.split_once('-'))
    else {
        return (printed, None);
    };
    match (min.trim().parse(), max.trim().parse()) {
        (Ok(min), Ok(max)) => (name.trim_end(), Some(Levels { min, max })),
        _ => (printed, None),
    }
}

/// What kind of place a section's tables belong to.
fn kind_of(section: &str) -> PlaceKind {
    match section {
        "keyRewards" => PlaceKind::Key,
        "sortieRewards" => PlaceKind::Sortie,
        "transientRewards" => PlaceKind::Transient,
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
    }

    #[test]
    fn enemy_sections_are_told_from_place_sections() {
        assert!(by_enemy("modByAvatar"));
        assert!(by_enemy("resourceByDrop"));
        assert!(!by_enemy("missionRewards"));
    }

    #[test]
    fn a_level_range_belongs_to_the_table_not_to_the_enemy() {
        assert_eq!(
            split_levels("Apex Membroid (Level 51 - 75)"),
            ("Apex Membroid", Some(Levels { min: 51, max: 75 }))
        );
        assert_eq!(
            split_levels("Tusk Thumper Bull"),
            ("Tusk Thumper Bull", None)
        );
        assert_eq!(
            split_levels("Corrupted Vor (Level 30)"),
            ("Corrupted Vor (Level 30)", None)
        );
    }
}
