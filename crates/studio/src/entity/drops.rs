use graph::{DropInfo, Graph, Levels, Node, PlaceKind, Rel};
use maud::{Markup, html};

use crate::fold::Fold;
use crate::page::{amount, card, col, dual, named, number, pct, plain};
use crate::words;

use super::farm;

/// Everywhere the item is played for: the mission tables that reward it, the enemies that
/// carry it, and the star-chart nodes the places point at.
pub fn render(graph: &Graph, id: &str) -> Markup {
    let places = from_places(graph, id);
    let enemies = from_enemies(graph, id);
    html! {
        (missions_card(graph, &places))
        (super::enemies::render(graph, &enemies))
        (farm::render(graph, &places))
        (yields(graph, id))
    }
}

/// A place's or an enemy's own table: everything it hands out. The item pages read this
/// relation the other way round, so without this those pages would say nothing about it.
fn yields(graph: &Graph, id: &str) -> Markup {
    let mut rows: Vec<(&str, &DropInfo)> = graph
        .from(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Drops(drop) => Some((e.to.as_str(), drop)),
            _ => None,
        })
        .collect();
    if rows.is_empty() {
        return html! {};
    }
    rows.sort_by(|(a, x), (b, y)| y.chance.total_cmp(&x.chance).then_with(|| a.cmp(b)));
    let turns = rows
        .iter()
        .any(|(_, d)| d.rotation.is_some() || d.stage.is_some());
    let ranges = rows.iter().any(|(_, d)| d.levels.is_some());
    let fold = Fold::new(rows.len());

    card(
        "Что здесь падает",
        Some(html! { span.card-n { (number(rows.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Предмет", "item"))
                    @if turns { (col("Ротация", "rotation")) }
                    @if ranges { (col("Уровень", "level")) }
                    (col("Редкость", "rarity"))
                    (col("Шанс", "chance"))
                    (col("Кол-во", "quantity"))
                    (col("В среднем", "avg. per roll"))
                } }
                tbody {
                    @for (at, (item, drop)) in rows.iter().enumerate() {
                        tr.folded[fold.hides(at)] {
                            td { (named(graph, item)) }
                            @if turns { td.dim { (turn(drop)) } }
                            @if ranges { td.num { (range(drop.levels)) } }
                            td.dim { (drop.rarity.to_lowercase()) }
                            td.num { (pct(drop.chance)) }
                            td.num { (count(drop)) }
                            td.num { (amount(drop.per_roll())) }
                        }
                    }
                }
            } }
        }),
    )
}

/// One line of a place's reward table.
pub struct PlaceRow<'a> {
    pub place: &'a str,
    pub kind: PlaceKind,
    pub drop: &'a DropInfo,
}

/// One line of an enemy's drop table, which the tables split by level range.
pub struct EnemyRow<'a> {
    pub enemy: &'a str,
    pub drop: &'a DropInfo,
}

/// Every place whose table rewards the item, richest first — the list is folded to its first
/// rows, so the best places have to be among them.
fn from_places<'a>(graph: &'a Graph, id: &str) -> Vec<PlaceRow<'a>> {
    let mut rows: Vec<PlaceRow<'a>> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match (&e.rel, graph.get(&e.from)) {
            (Rel::Drops(drop), Some(Node::Place(p))) => Some(PlaceRow {
                place: e.from.as_str(),
                kind: p.kind,
                drop,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.drop
            .per_roll()
            .total_cmp(&a.drop.per_roll())
            .then_with(|| a.place.cmp(b.place))
    });
    rows
}

/// Every enemy that carries the item, richest first.
fn from_enemies<'a>(graph: &'a Graph, id: &str) -> Vec<EnemyRow<'a>> {
    let mut rows: Vec<EnemyRow<'a>> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match (&e.rel, graph.get(&e.from)) {
            (Rel::Drops(drop), Some(Node::Enemy(_))) => Some(EnemyRow {
                enemy: e.from.as_str(),
                drop,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.drop
            .per_roll()
            .total_cmp(&a.drop.per_roll())
            .then_with(|| a.enemy.cmp(b.enemy))
    });
    rows
}

/// The reward tables of missions, keys, sorties and bounties.
fn missions_card(graph: &Graph, rows: &[PlaceRow<'_>]) -> Markup {
    if rows.is_empty() {
        return html! {};
    }
    let turns = rows
        .iter()
        .any(|r| r.drop.rotation.is_some() || r.drop.stage.is_some());
    let fold = Fold::new(rows.len());

    card(
        "Награды миссий",
        Some(html! { span.card-n { (number(rows.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Тип миссии", "mission type"))
                    (col("Источник", "source"))
                    @if turns { (col("Ротация", "rotation")) }
                    (col("Шанс", "chance"))
                    (col("Кол-во", "quantity"))
                    (col("В среднем", "avg. per roll"))
                    (col("Узлы карты", "star chart nodes"))
                } }
                tbody {
                    @for (at, row) in rows.iter().enumerate() {
                        tr.folded[fold.hides(at)] {
                            td { (mission(graph, row)) }
                            td { (plain(graph, row.place)) }
                            @if turns { td.dim { (turn(row.drop)) } }
                            td.num { (pct(row.drop.chance)) }
                            td.num { (count(row.drop)) }
                            td.num { (amount(row.drop.per_roll())) }
                            td { (farm::links(graph, row.place)) }
                        }
                    }
                }
            } }
        }),
    )
}

/// What is played on the place: the mission type of the node behind it, or failing that what
/// kind of table it is.
fn mission(graph: &Graph, row: &PlaceRow<'_>) -> Markup {
    match farm::region(graph, row.place) {
        Some(r) => dual(
            r.mission_label.ru.as_deref(),
            r.mission_label.en.as_deref().unwrap_or("—"),
        ),
        None => html! { span.dim { (words::place(row.kind)) } },
    }
}

/// The rotation the row sits in, with the bounty stage where the table is staged.
fn turn(drop: &DropInfo) -> Markup {
    html! {
        @match (&drop.rotation, &drop.stage) {
            (Some(rotation), Some(stage)) => { (rotation) span.bi-en { (stage) } }
            (Some(rotation), None) => (rotation),
            (None, Some(stage)) => (stage),
            (None, None) => "—",
        }
    }
}

/// The level range the table is printed for.
pub fn range(levels: Option<Levels>) -> Markup {
    html! {
        @match levels {
            Some(l) => { (l.min) "–" (l.max) }
            None => span.dim { "—" },
        }
    }
}

/// How many the row hands over. The tables print a stack size only where it is more than one.
pub fn count(drop: &DropInfo) -> Markup {
    html! {
        @match drop.count {
            Some(count) => { "×" (number(count)) }
            None => span.dim { "—" },
        }
    }
}
