use std::fmt::Display;

use consensus::{Conflict, Resolved, Source, Status};
use graph::Item;
use maud::{Markup, html};

use crate::page::{card, number, prov};
use crate::words;

/// The columns, in the order a person reads them.
const SOURCES: &[Source] = &[
    Source::Curated,
    Source::De,
    Source::Wiki,
    Source::Wfm,
    Source::Rule,
];

/// What every source says about the item, side by side, so a disagreement is visible
/// without leaving the page.
pub fn render(item: &Item, conflicts: &[&Conflict]) -> Markup {
    let name_ru = item.names.ru.as_ref();
    card(
        "Что говорят источники",
        Some(html! { span.card-n.hot[!conflicts.is_empty()] {
            (number(conflicts.len() as i64)) " расхожд."
        } }),
        html! {
            .scroll {
                table {
                    thead {
                        tr {
                            th { "Свойство" }
                            @for s in SOURCES { th { (words::source(*s)) } }
                            th { "Итог" }
                        }
                    }
                    tbody {
                        (row("name_en", &item.names.en, conflicts))
                        @if let Some(ru) = name_ru { (row("name_ru", ru, conflicts)) }
                        (row("category", &item.category, conflicts))
                        (row("prime", &item.prime, conflicts))
                        @if let Some(t) = &item.tradable { (row("tradable", t, conflicts)) }
                        @if let Some(v) = &item.vaulted { (row("vaulted", v, conflicts)) }
                        @if let Some(s) = &item.slug { (row("slug", s, conflicts)) }
                    }
                }
            }
            @if !conflicts.is_empty() {
                p.note {
                    "Расхождение можно закрепить в разделе "
                    a href="/conflicts" { "Конфликты" } "."
                }
            }
        },
    )
}

fn row<T: Display + Clone + PartialEq>(
    prop: &'static str,
    value: &Resolved<T>,
    conflicts: &[&Conflict],
) -> Markup {
    let said = claims(prop, value, conflicts);
    html! {
        tr {
            td { (words::prop(prop)) }
            @for s in SOURCES {
                td.dim {
                    @match said.iter().find(|(c, _)| c == s) {
                        Some((_, v)) => (v),
                        None => "—",
                    }
                }
            }
            td {
                (value.value.to_string())
                " " (badge(value.status))
            }
        }
    }
}

/// What each source claimed. Where they agree the resolver keeps only the value, so the
/// agreeing sources all show it; where they differ the conflict carries each one.
fn claims<T: Display + Clone + PartialEq>(
    prop: &str,
    value: &Resolved<T>,
    conflicts: &[&Conflict],
) -> Vec<(Source, String)> {
    match conflicts.iter().find(|c| c.prop == prop) {
        Some(c) => c.claims.clone(),
        None => value
            .sources
            .iter()
            .map(|s| (*s, value.value.to_string()))
            .collect(),
    }
}

fn badge(status: Status) -> Markup {
    html! {
        @match status {
            Status::Confirmed => span.tag.trade { (words::status(status)) },
            Status::Conflict => span.tag.bad { (words::status(status)) },
            Status::Single => span.tag { (words::status(status)) },
        }
    }
}

/// The provenance legend, shown once under the table.
pub fn legend() -> Markup {
    html! {
        .tags {
            @for s in SOURCES {
                span.tag { (prov(*s)) " " (words::source_full(*s)) }
            }
        }
    }
}
