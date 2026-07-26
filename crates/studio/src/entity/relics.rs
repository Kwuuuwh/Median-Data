use graph::{Extra, Graph, Node, Rel};
use maud::{Markup, html};

use crate::fold::Fold;
use crate::page::{card, col, named, number, pct};
use crate::words;

/// What a relic hands out, and the relics that hand out this item.
pub fn render(graph: &Graph, id: &str) -> Markup {
    html! {
        (rewards(graph, id))
        (from_relics(graph, id))
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

    let refinement = refinement_of(graph, id);
    let sum: f64 = rows
        .iter()
        .filter_map(|(_, rarity)| graph::chance(rarity, refinement))
        .sum();
    let fold = Fold::new(rows.len());

    card(
        "Награды реликвии",
        Some(html! { span.card-n.hot[!(0.995..=1.005).contains(&sum)] {
            "Σ " (pct(sum))
        } }),
        html! {
            p.why { "Улучшение: " (words::refinement(refinement)) }
            (fold.wrap(html! {
                .scroll { table {
                    thead { tr {
                        (col("Награда", "reward"))
                        (col("Редкость", "rarity"))
                        (col("Шанс", "chance"))
                    } }
                    tbody {
                        @for (at, (reward, rarity)) in rows.iter().enumerate() {
                            tr.folded[fold.hides(at)] {
                                td { (named(graph, reward)) }
                                td.dim { (rarity.to_lowercase()) }
                                td.num { (chance(rarity, refinement)) }
                            }
                        }
                    }
                } }
            }))
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
    if rows.is_empty() {
        return html! {};
    }
    let fold = Fold::new(rows.len());

    card(
        "Выпадает из реликвий",
        Some(html! { span.card-n { (number(rows.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Реликвия", "relic"))
                    (col("Улучшение", "refinement"))
                    (col("Редкость", "rarity"))
                    (col("Шанс", "chance"))
                } }
                tbody {
                    @for (at, (relic, rarity)) in rows.iter().enumerate() {
                        @let refinement = refinement_of(graph, relic);
                        tr.folded[fold.hides(at)] {
                            td { (named(graph, relic)) }
                            td.dim { (words::refinement(refinement)) }
                            td.dim { (rarity.to_lowercase()) }
                            td.num { (chance(rarity, refinement)) }
                        }
                    }
                }
            } }
        }),
    )
}

/// The chance a rarity carries at one refinement, where the reference table gives one.
fn chance(rarity: &str, refinement: &str) -> String {
    match graph::chance(rarity, refinement) {
        Some(c) => pct(c),
        None => "—".to_string(),
    }
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
