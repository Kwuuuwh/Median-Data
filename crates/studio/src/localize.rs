use std::collections::BTreeMap;

use maud::{Markup, html};
use serde::Deserialize;

use crate::list::{self, Query};
use crate::page::{Side, bar, card, encode, number, prov, shell, slug, stat};
use crate::state::Snapshot;
use crate::terms::{self, Row, TARGETS};
use crate::words;

pub use crate::terms::pending;

/// What is being translated, and whether the screen is showing work or corrections.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub what: String,
    /// Empty for what still needs a name, `done` for what already has one.
    #[serde(default)]
    pub show: String,
}

impl Filter {
    fn what(&self) -> &str {
        match self.what.is_empty() {
            true => "item",
            false => self.what.as_str(),
        }
    }

    /// The list is a to-do list by default: what is already translated is not work, and
    /// putting it in the same list makes it look like everything needs checking.
    fn done(&self) -> bool {
        self.show == "done"
    }

    /// Names judged to stay as the game writes them.
    fn kept(&self) -> bool {
        self.show == "kept"
    }

    fn hidden(&self) -> Vec<(&str, &str)> {
        let mut out = vec![("what", self.what())];
        if !self.show.is_empty() {
            out.push(("show", self.show.as_str()));
        }
        out
    }
}

/// Everything the app will show in Russian, and what is still English. Not only items: the
/// places, the star chart, the vendors, the labs, the mission types, and the words our own
/// tree groups the catalog by.
pub fn render(snap: &Snapshot, filter: &Filter, q: &Query) -> Markup {
    let what = filter.what();
    let done = filter.done();
    let kept = filter.kept();
    let mut rows = terms::rows(snap, what);
    rows.retain(|r| match (done, kept) {
        (_, true) => r.verbatim.is_some(),
        (true, _) => r.ru.is_some(),
        _ => r.ru.is_none() && r.verbatim.is_none(),
    });
    rows.retain(|r| {
        q.matches(&format!(
            "{} {} {}",
            r.en,
            r.ru.clone().unwrap_or_default(),
            r.key
        ))
    });
    let page = q.page(rows);
    let hidden = filter.hidden();

    shell(
        "Локализация",
        &Side::of(snap, "localize"),
        html! {
            (bar(
                html! {
                    a href="/" { "каталог" } span.sep { "›" }
                    a href="/localize" { "Локализация" }
                    span.sep { "›" } span.cur { (words::term(what)) }
                },
                Some(html! { span.chip.hot[pending(snap) > 0] {
                    (number(pending(snap) as i64)) " без русского"
                } }),
            ))
            .wrap {
                h1 { "Локализация" }
                p.why {
                    "Приложение показывает русский. Здесь видно, чем он подкреплён: имя от DE, "
                    "имя от рынка, имя, выведенное правилом, и имя, написанное рукой — это "
                    "разные вещи, и только последнее является переводом, за который кто-то отвечает."
                }

                (coverage(snap))
                h2 { "Предметы: чем подкреплён русский" }
                (by_winner(snap))
                (by_class(snap))

                h2 { "Работа" }
                .chips {
                    @for target in TARGETS {
                        @let (_, missing) = terms::tally(snap, target);
                        a class=@if what == target { "pick on" } @else { "pick" }
                          href={ "/localize?what=" (encode(target))
                                 (if done { "&show=done" } else { "" }) } {
                            (words::term(target)) " "
                            span.num.bad[missing > 0] { (number(missing as i64)) }
                        }
                    }
                }
                .chips.sub {
                    @let base = format!("/localize?what={}", encode(what));
                    a class=@if done || kept { "pick" } @else { "pick on" }
                      href=(base) { "нужен перевод" }
                    a class=@if done { "pick on" } @else { "pick" }
                      href={ (base) "&show=done" } { "правки" }
                    @let judged = snap.decided.verbatim.iter()
                        .filter(|(kind, _, _)| kind == what).count();
                    a class=@if kept { "pick on" } @else { "pick" }
                      href={ (base) "&show=kept" } {
                        "без перевода " span.num { (number(judged as i64)) }
                    }
                }
                (list::controls("/localize", q, &hidden, "имя или ключ…"))

                @if page.is_empty() {
                    .empty {
                        @if kept { "Ни одно имя не оставлено английским." }
                        @else if done { "Здесь пока ничего не переведено." }
                        @else { "Переводить нечего — всё названо." }
                    }
                } @else {
                    .scroll { table.rowset {
                        thead { tr {
                            th { "Английское имя" } th { "Что это" } th { "Русское имя" }
                        } }
                        tbody {
                            @for row in &page.rows { (line(what, row, &filter.show)) }
                        }
                    } }
                }
                (list::pager("/localize", q, &hidden, &page))

                h2 id="done" { "Написано рукой" }
                (decided(snap))
            }
        },
    )
}

/// One thing to translate, with the field that writes its name. The key it is stored under is
/// not shown: for an item it is a DE path, and the entity page behind the name carries it. A
/// row keeps its place in the to-do list after a name is written, so it is marked as settled
/// rather than looking like work that is still waiting.
pub fn line(what: &str, row: &Row, show: &str) -> Markup {
    let action = if what == "item" { "/name" } else { "/term" };
    let key = |markup: Markup| {
        html! {
            @if what == "item" {
                input type="hidden" name="item" value=(row.key);
            } @else {
                input type="hidden" name="kind" value=(what);
                input type="hidden" name="key" value=(row.key);
            }
            input type="hidden" name="frag" value="localize";
            input type="hidden" name="show" value=(show);
            (markup)
        }
    };
    html! {
        tr id={ "t-" (slug(what)) "-" (slug(&row.key)) } {
            td {
                @match terms::node_id(what, &row.key) {
                    Some(id) => (crate::page::link(&id, &row.en)),
                    None => (row.en),
                }
            }
            td.dim { (row.note) }
            td.ru {
                .ru-cell {
                    form.inline hx-post=(action) hx-target="closest tr" hx-swap="outerHTML"
                         action=(action) method="post" {
                        (key(html! {
                            input type="text" name="ru"
                                  value=(row.ru.clone().unwrap_or_default())
                                  placeholder="русское имя";
                            button type="submit" { "OK" }
                        }))
                    }
                    @if let Some(from) = row.from { (prov(from)) }
                    @match (&row.verbatim, &row.ru) {
                        (Some(_), _) => span.tag { "без перевода" },
                        (None, None) => span.tag.bad { "нет" },
                        (None, Some(_)) => {
                            @if show != "done" { span.tag.trade { "записано" } }
                        }
                    }
                    @if what != "item" {
                        @let verdict = match row.verbatim {
                            Some(_) => "/unverbatim",
                            None => "/verbatim",
                        };
                        form.inline hx-post=(verdict) hx-target="closest tr"
                             hx-swap="outerHTML" action=(verdict) method="post" {
                            (key(html! {
                                button.plain type="submit" {
                                    @if row.verbatim.is_some() { "Вернуть в перевод" }
                                    @else { "Не переводится" }
                                }
                            }))
                        }
                    }
                }
            }
        }
    }
}

/// How much of each kind of name is covered. The big number is what is still missing, since
/// that is the number that means work; the whole count sits beside the label.
fn coverage(snap: &Snapshot) -> Markup {
    html! {
        .stats {
            @for target in TARGETS {
                @let (total, missing) = terms::tally(snap, target);
                (stat(
                    &format!("{} · всего {}", words::term(target), number(total as i64)),
                    number(missing as i64),
                    if missing == 0 { "ok" } else { "bad" },
                    Some(&format!("/localize?what={}", encode(target))),
                ))
            }
        }
        p.note { "Число — сколько ещё без русского имени." }
    }
}

/// What the Russian side of the items rests on. A name a rule derived is not a translation, so
/// counting it as covered would be a lie.
fn by_winner(snap: &Snapshot) -> Markup {
    let mut by: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut none = 0;
    for item in snap.graph.items() {
        match &item.names.ru {
            Some(ru) => *by.entry(words::source_full(ru.winner)).or_default() += 1,
            None => none += 1,
        }
    }
    html! {
        (card("Откуда русское имя", None, html! {
            table {
                thead { tr { th { "Источник" } th { "Предметов" } } }
                tbody {
                    @for (source, n) in &by {
                        tr { td { (source) } td.num { (number(*n as i64)) } }
                    }
                    tr {
                        td { "нет русского имени" }
                        td.num { span.tag.bad[none > 0] { (number(none as i64)) } }
                    }
                }
            }
        }))
        p.note {
            "«Правило сборки» — это выведенное имя вида «X (Чертеж)», а не перевод: "
            "оно правильное ровно настолько, насколько правилен перевод результата."
        }
    }
}

/// The same question one level down: which class rests on what. A class whose Russian comes
/// from a rule is a class nobody has translated — the count is the size of that debt.
fn by_class(snap: &Snapshot) -> Markup {
    use consensus::Source;
    let mut rows: BTreeMap<&str, [usize; 5]> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for item in snap.graph.items() {
        let class = snap.taxonomy.class_slug(&item.kind.value);
        let slot = rows.entry(class).or_default();
        let at = match item.names.ru.as_ref().map(|r| r.winner) {
            None => 4,
            Some(Source::Curated) => 0,
            Some(Source::De) => 1,
            Some(Source::Wfm) => 2,
            Some(_) => 3,
        };
        slot[at] += 1;
        if at == 4 {
            *kinds
                .entry(class)
                .or_default()
                .entry(item.kind.value.as_str())
                .or_default() += 1;
        }
    }
    html! {
        (card("Покрытие по классам", None, html! {
            .scroll { table {
                thead { tr {
                    th { "Класс" } th { "рука" } th { "DE" } th { "рынок" }
                    th { "правило" } th { "нет" } th { "подкатегории без имени" }
                } }
                tbody {
                    @for (class, counts) in &rows {
                        tr {
                            td { (class) }
                            @for n in counts.iter() { td.num { (number(*n as i64)) } }
                            td.dim {
                                @for (kind, n) in kinds.get(class).into_iter().flatten() {
                                    span.tag.bad {
                                        (snap.taxonomy.label(&graph::Kind::new(*kind), "ru"))
                                        " " span.num { (number(*n as i64)) }
                                    }
                                    " "
                                }
                            }
                        }
                    }
                }
            } }
        }))
    }
}

/// Everything written by hand, with the way back.
fn decided(snap: &Snapshot) -> Markup {
    let d = &snap.decided;
    html! {
        @if d.names.is_empty() && d.terms.is_empty() {
            .empty { "Пока ничего не написано рукой." }
        } @else {
            .scroll { table {
                thead { tr { th { "Что" } th { "Ключ" } th { "Русское имя" } th {} } }
                tbody {
                    @for (item, ru) in &d.names {
                        tr {
                            td { "предмет" }
                            td { (crate::page::link(item, item.rsplit('/').next().unwrap_or(item)))
                                 br; span.path { (item) } }
                            td { (ru) }
                            td.num { (undo("/name", &[("item", item), ("ru", "")])) }
                        }
                    }
                    @for (kind, key, ru) in &d.terms {
                        tr {
                            td { (words::term(kind)) }
                            td { code { (key) } }
                            td { (ru) }
                            td.num {
                                (undo("/term", &[("kind", kind), ("key", key), ("ru", "")]))
                            }
                        }
                    }
                }
            } }
        }
    }
}

fn undo(action: &str, fields: &[(&str, &str)]) -> Markup {
    html! {
        form.inline action=(action) method="post" {
            @for (name, value) in fields {
                input type="hidden" name=(name) value=(value);
            }
            button.plain type="submit" { "Снять" }
        }
    }
}

/// The row a person just decided about, for the fragment that replaces it.
pub fn row_of(snap: &Snapshot, what: &str, key: &str, show: &str) -> Markup {
    match terms::rows(snap, what).into_iter().find(|r| r.key == key) {
        Some(row) => line(what, &row, show),
        None => html! { tr { td colspan="3" { "строки нет в снимке: нужна пересборка" } } },
    }
}
