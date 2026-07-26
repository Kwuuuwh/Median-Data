use maud::{Markup, html};
use serde::Deserialize;

use crate::fuzzy::{self, Names};
use crate::list::{self, Query};
use crate::page::{self, Side, bar, encode, named, number, shell, slug};
use crate::state::{Snapshot, Unresolved};
use crate::words;

/// How many candidates a row offers before the rest is noise.
const OPTIONS: usize = 5;

/// The drop tables, whose names the coverage check also reports.
pub const DROPS: &str = "drops";

/// The sources that print names, in the order the screen lists them.
const SOURCES: [&str; 4] = ["market", DROPS, "vendor", "dojo"];

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
                (refused(snap))
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
                        span.tag {
                            (number(u.count as i64)) " "
                            (words::plural(u.count, "строка", "строки", "строк"))
                        }
                        " "
                    }
                    span.tag.kind { (words::origin(&u.source)) }
                }
            }
            .path { (u.key) }
            @if !u.hint.is_empty() { p.note { (u.hint) } }
            (search(&u.source, &u.key, &u.name))
            div id={ "cand-" (id) } { (candidates(snap, names, &u.source, &u.key, &u.name)) }
            (not_an_item(&u.source, &u.key))
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

/// The other verdict a name can get: it names nothing the catalog can hold — a pickup a
/// player never owns, a bundle, a counter — so there is no item to look for.
fn not_an_item(source: &str, key: &str) -> Markup {
    html! {
        .curate-row.verdict {
            form.inline hx-post="/dismiss" hx-target={ "#" (key_id(source, key)) }
                 hx-swap="outerHTML" action="/dismiss" method="post" {
                input type="hidden" name="source" value=(source);
                input type="hidden" name="key" value=(key);
                input type="text" name="note" placeholder="чем это на самом деле является";
                button.plain type="submit" { "Это не предмет" }
            }
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
                            " " (marked(snap, hit, query))
                            .path { (hit.path) }
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

/// The row after a name is declared to be no item: what it really is, and the way back.
pub fn dropped(source: &str, key: &str, note: &str) -> Markup {
    html! {
        li.row.done id=(key_id(source, key)) {
            .row-h {
                span.row-t { (key) }
                span {
                    span.tag { "не предмет" } " "
                    span.tag.kind { (words::origin(source)) }
                }
            }
            .opt {
                .grow {
                    p.note { @if note.is_empty() { "без пояснения" } @else { (note) } }
                }
                form.inline action="/undismiss" method="post" {
                    input type="hidden" name="source" value=(source);
                    input type="hidden" name="key" value=(key);
                    button.plain type="submit" { "Вернуть" }
                }
            }
        }
    }
}

/// A candidate as its picture, its Russian name and its English one. The marking goes on the
/// English name: that is what the sources print and what the query was scored against.
fn marked(snap: &Snapshot, hit: &fuzzy::Hit, query: &str) -> Markup {
    let (primary, secondary) = page::names(&snap.graph, &hit.path);
    let href = format!("/entity?q={}", encode(&hit.path));
    html! {
        a.iref href=(href) {
            img.iref-icon src={ "/icon?q=" (encode(&hit.path)) "&lang=ru" } alt="" loading="lazy";
            span.iref-text {
                @match secondary {
                    Some(en) => {
                        span.iref-ru { (primary) }
                        span.iref-en { (fuzzy::diff(en, query)) }
                    }
                    None => span.iref-ru { (fuzzy::diff(primary, query)) },
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

/// Names declared to be no item at all, each with the way back.
fn refused(snap: &Snapshot) -> Markup {
    let rows = &snap.decided.dismissed;
    html! {
        @if !rows.is_empty() {
            h2 id="nothing" { "Не предметы" }
            p.why {
                (number(rows.len() as i64)) " "
                (words::plural(rows.len(), "имя названо", "имени названы", "имён названы"))
                " тем, чего каталог не держит. Проверки о них больше не спрашивают, но "
                "по-прежнему их находят."
            }
            .scroll { table {
                thead { tr {
                    th { "Источник" } th { "Имя" } th { "Что это на самом деле" } th {}
                } }
                tbody {
                    @for (source, key, note) in rows {
                        tr {
                            td { (words::origin(source)) }
                            td { code { (key) } }
                            td.dim {
                                @if note.is_empty() { "без пояснения" } @else { (note) }
                            }
                            td.num {
                                form.inline action="/undismiss" method="post" {
                                    input type="hidden" name="source" value=(source);
                                    input type="hidden" name="key" value=(key);
                                    button.plain type="submit" { "Вернуть" }
                                }
                            }
                        }
                    }
                }
            } }
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
