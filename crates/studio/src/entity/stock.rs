use graph::{Graph, Offer, Rel};
use maud::{Markup, html};

use crate::page::{card, named, number};
use crate::words;

/// What a vendor hands over. An edge means the item has been on offer, never that it is in
/// stock now — the counter, the price and the rank are what stays true between visits.
pub fn sold_by(graph: &Graph, id: &str) -> Markup {
    let offers: Vec<(&str, &Offer)> = graph
        .from(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Sells(offer) => Some((e.to.as_str(), offer)),
            _ => None,
        })
        .collect();

    html! {
        @if !offers.is_empty() {
            (card("Ассортимент", Some(html! { span.card-n { (offers.len()) } }), html! {
                .scroll { table {
                    thead { tr {
                        th { "Предмет" } th { "Цена" } th { "Прилавок" } th { "Условия" }
                    } }
                    tbody {
                        @for (item, offer) in &offers {
                            tr {
                                td { (named(graph, item)) }
                                td { (price(offer)) }
                                td.dim { (offer.store.as_deref().unwrap_or("—")) }
                                td.dim { (terms(offer)) }
                            }
                        }
                    }
                } }
            }))
        }
    }
}

/// Where an item can be bought, for its own page.
pub fn sold_at(graph: &Graph, id: &str) -> Markup {
    let offers: Vec<(&str, &Offer)> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match &e.rel {
            Rel::Sells(offer) => Some((e.from.as_str(), offer)),
            _ => None,
        })
        .collect();

    html! {
        @if !offers.is_empty() {
            (card("Где купить", Some(html! { span.card-n { (offers.len()) } }), html! {
                .scroll { table {
                    thead { tr {
                        th { "Торговец" } th { "Цена" } th { "Прилавок" } th { "Условия" }
                    } }
                    tbody {
                        @for (vendor, offer) in &offers {
                            tr {
                                td {
                                    (named(graph, vendor))
                                    @if let Some(graph::Node::Vendor(v)) = graph.get(vendor) {
                                        br; span.path { (words::vendor(v.kind.as_deref())) }
                                    }
                                }
                                td { (price(offer)) }
                                td.dim { (offer.store.as_deref().unwrap_or("—")) }
                                td.dim { (terms(offer)) }
                            }
                        }
                    }
                } }
            }))
        }
    }
}

/// What it costs, in whatever the counter takes.
fn price(offer: &Offer) -> Markup {
    html! {
        @match offer.cost {
            Some(cost) => {
                span.num { (number(cost)) }
                @if let Some(currency) = &offer.currency { " " span.dim { (currency) } }
            }
            None => span.dim { "—" },
        }
        @if let Some(credits) = offer.credits {
            span.dim { " + " (number(credits)) " кредитов" }
        }
        @if offer.count > 1 { span.tag { "×" (offer.count) } }
    }
}

/// What has to be true to buy it, and how the offer behaves over time.
fn terms(offer: &Offer) -> Markup {
    html! {
        @if let Some(rank) = offer.rank { span.tag { "ранг " (rank) } " " }
        @if let Some(timer) = offer.timer {
            span.tag { "меняется через " (words::duration(timer)) } " "
        }
        @if offer.always { span.tag.trade { "всегда" } " " }
        @if offer.gone { span.tag.bad { "больше не привозит" } " " }
        @if offer.times > 0 { span.dim { "привозил " (offer.times) " раз" } }
    }
}
