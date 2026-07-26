use std::collections::BTreeMap;

use graph::{Class, Item, Kind, Node};
use maud::{Markup, html};
use serde::Deserialize;

use crate::list::{self, Query};
use crate::page::{Side, bar, encode, named, number, shell};
use crate::state::Snapshot;

/// What the catalog is narrowed to: a class, one of its kinds, and one property worth
/// working through.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub kind: String,
    /// One of `ru`, `source`, `held`, `trade`, `craft` — or empty for everything.
    #[serde(default)]
    pub only: String,
}

impl Filter {
    fn hidden(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        if !self.class.is_empty() {
            out.push(("class", self.class.as_str()));
        }
        if !self.kind.is_empty() {
            out.push(("kind", self.kind.as_str()));
        }
        if !self.only.is_empty() {
            out.push(("only", self.only.as_str()));
        }
        out
    }

    fn url(&self, class: &str, kind: &str, only: &str) -> String {
        let mut out = format!("/catalog?class={}", encode(class));
        if !kind.is_empty() {
            out.push_str(&format!("&kind={}", encode(kind)));
        }
        if !only.is_empty() {
            out.push_str(&format!("&only={}", encode(only)));
        }
        out
    }
}

/// Everything the catalog holds, walked the way the app will show it: a class, then a kind,
/// then the items themselves — each row carrying what a person needs to spot a wrong one.
pub fn render(snap: &Snapshot, filter: &Filter, q: &Query) -> Markup {
    let counts = by_kind(snap);
    let class = snap
        .taxonomy
        .classes()
        .iter()
        .find(|c| c.slug == filter.class);
    let rows: Vec<&Item> = snap
        .graph
        .items()
        .filter(|i| keeps(snap, filter, i))
        .filter(|i| q.matches(&text(i)))
        .collect();
    let page = q.page(rows);
    let hidden = filter.hidden();

    shell(
        "Каталог",
        &Side::of(snap, "catalog"),
        html! {
            (bar(
                html! {
                    a href="/catalog" { "каталог" }
                    @if let Some(class) = class {
                        span.sep { "›" }
                        a href=(filter.url(&class.slug, "", &filter.only)) { (class.ru) }
                    }
                    @if !filter.kind.is_empty() {
                        span.sep { "›" }
                        span.cur { (snap.taxonomy.label(&Kind::new(&filter.kind), "ru")) }
                    }
                },
                Some(html! { span.chip { (number(page.total as i64)) " предметов" } }),
            ))
            .wrap {
                h1 { "Каталог" }
                p.why {
                    "Всё, что собралось из источников. Строка — это предмет: картинка, оба "
                    "имени, куда он отнесён, чем подтверждён и откуда берётся."
                }

                (classes(snap, filter, &counts))
                @if let Some(class) = class { (kinds(filter, class, &counts)) }
                (only(snap, filter))
                (list::controls("/catalog", q, &hidden, "имя или путь…"))

                @if page.is_empty() {
                    .empty { "Ни один предмет не подходит под эти условия." }
                } @else {
                    .scroll { table.rowset {
                        thead { tr {
                            th { "Предмет" } th { "Категория" } th { "Русское имя" }
                            th { "Откуда берётся" } th { "Рынок" }
                        } }
                        tbody {
                            @for item in &page.rows { (row(snap, item)) }
                        }
                    } }
                }
                (list::pager("/catalog", q, &hidden, &page))
            }
        },
    )
}

/// One item as the catalog shows it. Also served on its own after an inline decision, so the
/// row a person edited is the only thing that changes.
pub fn row(snap: &Snapshot, item: &Item) -> Markup {
    let sources = crate::sources::of(&snap.graph, &item.unique_name);
    html! {
        tr id={ "row-" (crate::page::slug(&item.unique_name)) } {
            td {
                (named(&snap.graph, &item.unique_name))
                .path { (item.unique_name) }
            }
            td {
                span.tag.kind { (snap.taxonomy.label(&item.kind.value, "ru")) }
                @if item.kind.value.is_unknown() { span.tag.bad { "не определено" } }
                br;
                span.path { (snap.taxonomy.class_label(&item.kind.value, "ru")) }
            }
            td { (ru_cell(item)) }
            td.dim {
                @if sources.is_empty() {
                    span.tag.bad { "неизвестно" }
                } @else {
                    @for (what, count) in &sources {
                        span.tag { (what) " " span.num { (number(*count as i64)) } } " "
                    }
                }
            }
            td.dim {
                @match &item.tradable {
                    Some(t) if t.value => span.tag.trade { "торгуется" },
                    Some(_) => span.tag { "нет" },
                    None => span.tag { "?" },
                }
                @if let Some(d) = item.ducats { " " span.tag { (number(d)) " дук." } }
                @if snap.scope.reason(&item.unique_name).is_some() {
                    " " span.tag.bad { "не в поставке" }
                }
            }
        }
    }
}

/// The Russian name, or the field to write one in. A name a rule derived is shown as such —
/// it is a translation nobody checked.
fn ru_cell(item: &Item) -> Markup {
    html! {
        @match &item.names.ru {
            Some(ru) => {
                (ru.value)
                " " (crate::page::prov(ru.winner))
            }
            None => {
                form.inline hx-post="/name" hx-target="closest tr" hx-swap="outerHTML"
                     action="/name" method="post" {
                    input type="hidden" name="item" value=(item.unique_name);
                    input type="hidden" name="frag" value="catalog";
                    input type="text" name="ru" placeholder="русское имя";
                    button type="submit" { "OK" }
                }
            }
        }
    }
}

/// Every class with how many items it holds.
fn classes(snap: &Snapshot, filter: &Filter, counts: &BTreeMap<&str, usize>) -> Markup {
    html! {
        .chips {
            a class=@if filter.class.is_empty() { "pick on" } @else { "pick" }
              href={ "/catalog" (only_query(&filter.only)) } { "все" }
            @for class in snap.taxonomy.classes() {
                @let total: usize = class.kind.iter()
                    .map(|k| counts.get(k.slug.as_str()).copied().unwrap_or(0)).sum();
                a class=@if filter.class == class.slug { "pick on" } @else { "pick" }
                  href=(filter.url(&class.slug, "", &filter.only)) {
                    (class.ru) " " span.num { (number(total as i64)) }
                }
            }
        }
    }
}

/// The kinds of one class.
fn kinds(filter: &Filter, class: &Class, counts: &BTreeMap<&str, usize>) -> Markup {
    html! {
        .chips.sub {
            a class=@if filter.kind.is_empty() { "pick on" } @else { "pick" }
              href=(filter.url(&class.slug, "", &filter.only)) { "весь класс" }
            @for leaf in &class.kind {
                @let n = counts.get(leaf.slug.as_str()).copied().unwrap_or(0);
                a class=@if filter.kind == leaf.slug { "pick on" } @else { "pick" }
                  href=(filter.url(&class.slug, &leaf.slug, &filter.only)) {
                    (leaf.ru) " " span.num { (number(n as i64)) }
                }
            }
        }
    }
}

/// The properties worth working through, each with how many items it leaves.
fn only(snap: &Snapshot, filter: &Filter) -> Markup {
    let choices = [
        ("", "всё"),
        ("ru", "без русского имени"),
        ("source", "неизвестно, откуда"),
        ("trade", "торгуется"),
        ("craft", "есть рецепт"),
        ("held", "не в поставке"),
    ];
    html! {
        .chips.sub {
            @for (key, label) in choices {
                @let narrowed = Filter { only: key.to_string(), ..filter.clone() };
                @let n = snap.graph.items().filter(|i| keeps(snap, &narrowed, i)).count();
                a class=@if filter.only == key { "pick on" } @else { "pick" }
                  href=(filter.url(&filter.class, &filter.kind, key)) {
                    (label) " " span.num { (number(n as i64)) }
                }
            }
        }
    }
}

fn only_query(only: &str) -> String {
    if only.is_empty() {
        String::new()
    } else {
        format!("?only={}", encode(only))
    }
}

/// Whether an item passes the class, kind and property the screen is narrowed to.
fn keeps(snap: &Snapshot, filter: &Filter, item: &Item) -> bool {
    if !filter.kind.is_empty() && item.kind.value.as_str() != filter.kind {
        return false;
    }
    if !filter.class.is_empty() && snap.taxonomy.class_slug(&item.kind.value) != filter.class {
        return false;
    }
    match filter.only.as_str() {
        "ru" => item.names.ru.is_none(),
        "source" => {
            snap.taxonomy.sourced(&item.kind.value)
                && !graph::obtainable(&snap.graph, &item.unique_name)
        }
        "trade" => item.tradable.as_ref().is_some_and(|t| t.value),
        "craft" => graph::producer(&snap.graph, &item.unique_name).is_some(),
        "held" => snap.scope.reason(&item.unique_name).is_some(),
        _ => true,
    }
}

/// What the filter box searches: both names and the path.
fn text(item: &Item) -> String {
    let ru = item
        .names
        .ru
        .as_ref()
        .map(|r| r.value.as_str())
        .unwrap_or("");
    format!("{} {} {}", item.names.en.value, ru, item.unique_name)
}

fn by_kind(snap: &Snapshot) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for item in snap.graph.items() {
        *counts.entry(item.kind.value.as_str()).or_default() += 1;
    }
    counts
}

/// The item behind a path, when it is one.
pub fn item<'a>(snap: &'a Snapshot, path: &str) -> Option<&'a Item> {
    match snap.graph.get(path) {
        Some(Node::Item(item)) => Some(item),
        _ => None,
    }
}
