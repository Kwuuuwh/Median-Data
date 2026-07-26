use std::collections::BTreeSet;

use graph::{Graph, Node, Region, Rel};
use maud::{Markup, html};

use crate::fold::Fold;
use crate::page::{card, col, dual, encode, number, plain};

use super::drops::Row;

/// The star-chart nodes worth going to, one row per node however many of its tables the item
/// sits in. The rows arrive richest first, so a node keeps the place of its best table.
/// Enemies are left out: nothing says where they spawn.
pub fn render(graph: &Graph, rows: &[Row<'_>]) -> Markup {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut nodes: Vec<&Region> = Vec::new();
    for row in rows {
        for id in at(graph, row.place) {
            if seen.insert(id)
                && let Some(region) = region_at(graph, id)
            {
                nodes.push(region);
            }
        }
    }
    if nodes.is_empty() {
        return html! {};
    }
    let fold = Fold::new(nodes.len());

    card(
        "Где фармить",
        Some(html! { span.card-n { (number(nodes.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Цель", "target"))
                    (col("Планета", "planet"))
                    (col("Узел", "name"))
                    (col("Тип", "type"))
                    (col("Уровень", "level"))
                    (col("Тайлсет", "tile set"))
                } }
                tbody {
                    @for (at, region) in nodes.iter().enumerate() {
                        tr.folded[fold.hides(at)] {
                            td { (target(region)) }
                            td { (dual(region.planet_ru.as_deref(), &region.planet)) }
                            td { (plain(graph, &graph::region_id(&region.node))) }
                            td { (label(&region.mission_label)) }
                            td.num { (level(region)) }
                            td { (label(&region.tileset)) }
                        }
                    }
                }
            } }
        }),
    )
}

/// The star-chart nodes one place names, as links.
pub fn links(graph: &Graph, place: &str) -> Markup {
    let nodes = at(graph, place);
    html! {
        @if nodes.is_empty() {
            span.dim { "—" }
        } @else {
            @for node in &nodes { (plain(graph, node)) }
        }
    }
}

/// The star-chart node behind a place, where the place names one.
pub fn region<'a>(graph: &'a Graph, place: &str) -> Option<&'a Region> {
    at(graph, place).first().and_then(|id| region_at(graph, id))
}

/// Node ids of every star-chart node a place is played on.
fn at<'a>(graph: &'a Graph, place: &str) -> Vec<&'a str> {
    graph
        .from(place)
        .into_iter()
        .filter(|e| e.rel == Rel::At)
        .map(|e| e.to.as_str())
        .collect()
}

fn region_at<'a>(graph: &'a Graph, id: &str) -> Option<&'a Region> {
    match graph.get(id) {
        Some(Node::Region(r)) => Some(r),
        _ => None,
    }
}

/// Who holds the node, with the emblem the game draws for them.
fn target(region: &Region) -> Markup {
    let label = &region.faction_label;
    html! {
        span.target {
            @if let Some(icon) = &label.icon {
                img.target-icon src={ "/icon?q=" (encode(icon)) } alt="" loading="lazy";
            }
            @match label.en.as_deref() {
                Some(en) => (dual(label.ru.as_deref(), en)),
                None => span.dim { "не расшифровано: " (region.faction) },
            }
        }
    }
}

/// A label in Russian over its English original, or a dash where nothing names it.
fn label(label: &graph::Label) -> Markup {
    match label.en.as_deref() {
        Some(en) => dual(label.ru.as_deref(), en),
        None => html! { span.dim { "—" } },
    }
}

/// The enemy levels the node runs at.
fn level(region: &Region) -> Markup {
    html! {
        @if region.max_level > 0 {
            (region.min_level) "–" (region.max_level)
        } @else {
            span.dim { "—" }
        }
    }
}
