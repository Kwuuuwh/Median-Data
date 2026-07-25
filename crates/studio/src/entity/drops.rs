use graph::{Extra, Graph, Node, Rel};
use maud::{Markup, html};

use crate::page::{card, link, pct};
use crate::words;

use super::craft::{label, named};

/// Where the item comes from, and — for a relic — what it hands out.
pub fn render(graph: &Graph, id: &str) -> Markup {
    html! {
        (rewards(graph, id))
        (from_relics(graph, id))
        (from_places(graph, id))
    }
}

/// A relic's own reward table, with the chance each refinement gives.
fn rewards(graph: &Graph, id: &str) -> Markup {
    let rows: Vec<(&str, &str)> = graph
        .from(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Rewards { rarity } => Some((e.to.as_str(), rarity.as_str())),
            _ => None,
        })
        .collect();
    if rows.is_empty() {
        return html! {};
    }

    let refinement = match graph.get(id) {
        Some(Node::Item(item)) => match &item.extra {
            Extra::Relic(r) => r.refinement.as_str(),
            Extra::None => "intact",
        },
        _ => "intact",
    };
    let sum: f64 = rows
        .iter()
        .filter_map(|(_, rarity)| graph::chance(rarity, refinement))
        .sum();

    card(
        "Награды реликвии",
        Some(html! { span.card-n.hot[!(0.995..=1.005).contains(&sum)] {
            "Σ " (pct(sum))
        } }),
        html! {
            p.why { "Улучшение: " (words::refinement(refinement)) }
            .scroll { table {
                thead { tr { th { "Награда" } th { "Редкость" } th { "Шанс" } } }
                tbody {
                    @for (reward, rarity) in &rows {
                        tr {
                            td { (named(graph, reward)) }
                            td.dim { (rarity.to_lowercase()) }
                            td.num {
                                @match graph::chance(rarity, refinement) {
                                    Some(c) => (pct(c)),
                                    None => "—",
                                }
                            }
                        }
                    }
                }
            } }
        },
    )
}

/// The relics that can award this item, one row per refinement it is worth opening.
fn from_relics(graph: &Graph, id: &str) -> Markup {
    let rows: Vec<(&str, &str)> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Rewards { rarity } => Some((e.from.as_str(), rarity.as_str())),
            _ => None,
        })
        .collect();

    html! {
        @if !rows.is_empty() {
            (card("Выпадает из реликвий", Some(html! { span.card-n { (rows.len()) } }), html! {
                .scroll { table {
                    thead { tr {
                        th { "Реликвия" } th { "Улучшение" } th { "Редкость" } th { "Шанс" }
                    } }
                    tbody {
                        @for (relic, rarity) in &rows {
                            @let refinement = refinement_of(graph, relic);
                            tr {
                                td { (named(graph, relic)) }
                                td.dim { (words::refinement(refinement)) }
                                td.dim { (rarity.to_lowercase()) }
                                td.num {
                                    @match graph::chance(rarity, refinement) {
                                        Some(c) => (pct(c)),
                                        None => "—",
                                    }
                                }
                            }
                        }
                    }
                } }
            }))
        }
    }
}

/// The missions, keys and enemies that drop this item.
fn from_places(graph: &Graph, id: &str) -> Markup {
    let rows: Vec<(&str, &graph::DropInfo)> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Drops(d) => Some((e.from.as_str(), d)),
            _ => None,
        })
        .collect();
    if rows.is_empty() {
        return html! {};
    }

    let rotations = rows.iter().any(|(_, d)| d.rotation.is_some());
    let stages = rows.iter().any(|(_, d)| d.stage.is_some());
    let tables = rows.iter().any(|(_, d)| d.table_chance.is_some());

    card(
        "Где падает",
        Some(html! { span.card-n { (rows.len()) } }),
        html! {
            .scroll { table {
                thead { tr {
                    th { "Место" }
                    th { "Тип" }
                    @if rotations { th { "Ротация" } }
                    @if stages { th { "Стадия" } }
                    th { "Редкость" }
                    @if tables { th { "Шанс таблицы" } }
                    th { "Шанс" }
                } }
                tbody {
                    @for (place, d) in &rows {
                        tr {
                            td { (link(place, label(graph, place))) }
                            td.dim { (kind_of(graph, place)) }
                            @if rotations { td.dim { (d.rotation.as_deref().unwrap_or("—")) } }
                            @if stages { td.dim { (d.stage.as_deref().unwrap_or("—")) } }
                            td.dim { (d.rarity.to_lowercase()) }
                            @if tables {
                                td.num {
                                    @match d.table_chance {
                                        Some(c) => (pct(c)),
                                        None => "—",
                                    }
                                }
                            }
                            td.num { (pct(d.chance)) }
                        }
                    }
                }
            } }
        },
    )
}

fn refinement_of<'a>(graph: &'a Graph, relic: &str) -> &'a str {
    match graph.get(relic) {
        Some(Node::Item(item)) => match &item.extra {
            Extra::Relic(r) => r.refinement.as_str(),
            Extra::None => "intact",
        },
        _ => "intact",
    }
}

fn kind_of(graph: &Graph, place: &str) -> &'static str {
    match graph.get(place) {
        Some(Node::Place(p)) => words::place(p.kind),
        _ => "—",
    }
}
