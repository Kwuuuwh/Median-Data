use graph::{DropInfo, Graph, Node, PlaceKind, Rel};
use maud::{Markup, html};

use crate::fold::Fold;
use crate::page::{amount, card, col, dual, named, number, pct, plain};
use crate::words;

use super::farm;

/// Everywhere the item is played for: the mission tables that reward it, the enemies that
/// carry it, and the star-chart nodes both point at.
pub fn render(graph: &Graph, id: &str) -> Markup {
    let rows = falls_from(graph, id);
    let (enemies, missions): (Vec<Row<'_>>, Vec<Row<'_>>) =
        rows.into_iter().partition(|r| r.kind == PlaceKind::Enemy);
    html! {
        (missions_card(graph, &missions))
        (super::enemies::render(graph, &enemies))
        (farm::render(graph, &missions))
        (yields(graph, id))
    }
}

/// A place's own table: everything it hands out. The item pages read this relation the other
/// way round, so without this a place page would say nothing about what it is for.
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
    let fold = Fold::new(rows.len());

    card(
        "Что здесь падает",
        Some(html! { span.card-n { (number(rows.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Предмет", "item"))
                    @if turns { (col("Ротация", "rotation")) }
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

/// One drop-table line: the place, what kind of place it is, and how the item falls out of it.
pub struct Row<'a> {
    pub place: &'a str,
    pub kind: PlaceKind,
    pub drop: &'a DropInfo,
}

/// Every place that drops the item, whatever kind of place it is, richest first — the list is
/// folded to its first rows, so the best places have to be among them.
fn falls_from<'a>(graph: &'a Graph, id: &str) -> Vec<Row<'a>> {
    let mut rows: Vec<Row<'a>> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Drops(drop) => Some(Row {
                place: e.from.as_str(),
                kind: kind_of(graph, e.from.as_str()),
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

/// The reward tables of missions, keys, sorties and bounties.
fn missions_card(graph: &Graph, rows: &[Row<'_>]) -> Markup {
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
fn mission(graph: &Graph, row: &Row<'_>) -> Markup {
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

/// How many the row hands over. The tables print a stack size only where it is more than one.
pub fn count(drop: &DropInfo) -> Markup {
    html! {
        @match drop.count {
            Some(count) => { "×" (number(count)) }
            None => span.dim { "—" },
        }
    }
}

fn kind_of(graph: &Graph, place: &str) -> PlaceKind {
    match graph.get(place) {
        Some(Node::Place(p)) => p.kind,
        _ => PlaceKind::Node,
    }
}
