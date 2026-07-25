use maud::{Markup, html};
use serde::Deserialize;

use crate::fuzzy::{self, Names};
use crate::list::{self, Query};
use crate::page::{Side, bar, encode, named, number, shell, slug};
use crate::state::{Snapshot, Unresolved};
use crate::words;

/// How many candidates a row offers before the rest is noise.
const OPTIONS: usize = 5;

/// The sources that print names, in the order the screen lists them.
const SOURCES: [&str; 4] = ["market", "drops", "vendor", "dojo"];

/// Which source's orphans the screen is showing.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub source: String,
}

/// Every name a source prints that no catalog item answers to, whatever the source. One
/// table, one decision: say which item it means, and the build resolves that name for every
/// source that prints it.
pub fn render(snap: &Snapshot, names: &Names, filter: &Filter, q: &Query) -> Markup {
    let rows: Vec<&Unresolved> = snap
        .unresolved
        .iter()
        .filter(|u| filter.source.is_empty() || u.source == filter.source)
        .filter(|u| q.matches(&format!("{} {} {}", u.name, u.key, u.hint)))
        .collect();
    let page = q.page(rows);
    let hidden: Vec<(&str, &str)> = match filter.source.is_empty() {
        true => Vec::new(),
        false => vec![("source", filter.source.as_str())],
    };

    shell(
        "Маппинг",
        &Side::of(snap, "mapping"),
        html! {
            (bar(
                html! {
                    a href="/" { "каталог" } span.sep { "›" } span.cur { "Маппинг" }
                    @if !filter.source.is_empty() {
                        span.sep { "›" } span.cur { (words::origin(&filter.source)) }
                    }
                },
                Some(html! { span.chip.hot[!snap.unresolved.is_empty()] {
                    (number(snap.unresolved.len() as i64)) " без предмета"
                } }),
            ))
            .wrap {
                h1 { "Имена без предмета" }
                p.why {
                    "Источники называют предметы печатным именем. Здесь то, что каталог "
                    "не смог отнести ни к чему: точные и однозначные совпадения сборка "
                    "делает сама и сюда не попадают. Процент — только подсказка, записывается "
                    "точная связь."
                }

                .chips {
                    a class=@if filter.source.is_empty() { "pick on" } @else { "pick" }
                      href="/mapping" {
                        "все " span.num { (number(snap.unresolved.len() as i64)) }
                    }
                    @for source in SOURCES {
                        @let n = snap.from_source(source).count();
                        a class=@if filter.source == source { "pick on" } @else { "pick" }
                          href={ "/mapping?source=" (encode(source)) } {
                            (words::origin(source)) " " span.num { (number(n as i64)) }
                        }
                    }
                }
                (list::controls("/mapping", q, &hidden, "имя, ключ или где встретилось…"))

                @if page.is_empty() {
                    .empty { "Связывать нечего." }
                } @else {
                    ul.rows {
                        @for u in &page.rows { (row(snap, names, u)) }
                    }
                }
                (list::pager("/mapping", q, &hidden, &page))

                h2 id="done" { "Уже связано" }
                (decided(snap))
            }
        },
    )
}

/// One unresolved name with what it might mean.
pub fn row(snap: &Snapshot, names: &Names, u: &Unresolved) -> Markup {
    let id = key_id(&u.source, &u.key);
    html! {
        li.row id=(id) {
            .row-h {
                span.row-t { (u.name) }
                span {
                    @if u.count > 1 {
                        span.tag { (u.count) " " (words::plural(u.count, "строка", "строки", "строк")) }
                        " "
                    }
                    span.tag.kind { (words::origin(&u.source)) }
                }
            }
            .path { (u.key) }
            @if !u.hint.is_empty() { p.note { (u.hint) } }
            (search(&u.source, &u.key, &u.name))
            div id={ "cand-" (id) } { (candidates(snap, names, &u.source, &u.key, &u.name)) }
        }
    }
}

/// The box for looking the item up by hand, which answers as it is typed.
fn search(source: &str, key: &str, name: &str) -> Markup {
    let target = format!("#cand-{}", key_id(source, key));
    let url = format!("/suggest?source={}&key={}", encode(source), encode(key));
    html! {
        .curate-row {
            input type="search" name="q" value=(name) placeholder="искать предмет…"
                  hx-get=(url) hx-target=(target) hx-swap="innerHTML"
                  hx-trigger="keyup changed delay:250ms, search";
        }
    }
}

/// The items a printed name might mean, best first.
pub fn candidates(snap: &Snapshot, names: &Names, source: &str, key: &str, query: &str) -> Markup {
    let hits = names.best(query, OPTIONS);
    html! {
        @if hits.is_empty() {
            p.note { "Ничего похожего. Попробуйте другое написание — связь можно записать только на существующий предмет." }
        } @else {
            .opts {
                @for hit in &hits {
                    .opt {
                        .grow {
                            span.score.sure[hit.score == 100] { (hit.score) "%" }
                            " " (named(&snap.graph, &hit.path))
                            .path { (fuzzy::diff(&hit.name, query)) " · " (hit.path) }
                        }
                        form.inline hx-post="/map" hx-target={ "#" (key_id(source, key)) }
                             hx-swap="outerHTML" action="/map" method="post" {
                            input type="hidden" name="source" value=(source);
                            input type="hidden" name="key" value=(key);
                            input type="hidden" name="item" value=(hit.path);
                            button type="submit" { "Связать" }
                        }
                    }
                }
            }
        }
    }
}

/// The row after a decision: what it was tied to, and the way back.
pub fn settled(snap: &Snapshot, source: &str, key: &str, item: &str) -> Markup {
    html! {
        li.row.done id=(key_id(source, key)) {
            .row-h {
                span.row-t { (key) }
                span.tag.kind { (words::origin(source)) }
            }
            .opt {
                .grow { (named(&snap.graph, item)) .path { (item) } }
                form.inline action="/unmap" method="post" {
                    input type="hidden" name="source" value=(source);
                    input type="hidden" name="key" value=(key);
                    button.plain type="submit" { "Снять" }
                }
            }
        }
    }
}

/// Every mapping on record, each with the way back.
fn decided(snap: &Snapshot) -> Markup {
    let links = &snap.decided.links;
    html! {
        @if links.is_empty() {
            .empty { "Пока ничего не связано вручную — всё, что есть, вывела сборка." }
        } @else {
            .scroll { table {
                thead { tr { th { "Источник" } th { "Имя" } th { "Предмет" } th {} } }
                tbody {
                    @for (source, key, item) in links {
                        tr {
                            td { (words::origin(source)) }
                            td { code { (key) } }
                            td { (named(&snap.graph, item)) br; span.path { (item) } }
                            td.num {
                                form.inline action="/unmap" method="post" {
                                    input type="hidden" name="source" value=(source);
                                    input type="hidden" name="key" value=(key);
                                    button.plain type="submit" { "Снять" }
                                }
                            }
                        }
                    }
                }
            } }
        }
    }
}

/// A row's element id, stable for one source and key.
pub fn key_id(source: &str, key: &str) -> String {
    format!("m-{}-{}", slug(source), slug(key))
}
