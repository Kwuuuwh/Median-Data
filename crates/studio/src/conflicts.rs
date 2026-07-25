use std::collections::BTreeMap;

use consensus::{Conflict, Source};
use graph::{Item, Node};
use maud::{Markup, html};

use crate::list::{self, Query};
use crate::page::{Side, bar, encode, number, prov, shell, stat};
use crate::state::Snapshot;
use crate::words;

/// Where sources disagree, one card per item, every claim on one line.
pub fn render(snap: &Snapshot, prop: Option<&str>, settled: Option<&str>, q: &Query) -> Markup {
    let by_prop = tally(snap);
    let grouped: Vec<(&str, Vec<&Conflict>)> = group(snap, prop)
        .into_iter()
        .filter(|(path, found)| q.matches(path) || found.iter().any(|c| q.matches(&c.chosen)))
        .collect();
    let hidden: Vec<(&str, &str)> = prop.map(|p| vec![("prop", p)]).unwrap_or_default();
    let page = q.page(grouped);

    shell(
        "Конфликты",
        &Side::of(snap, "conflicts"),
        html! {
            (bar(
                html! {
                    a href="/" { "каталог" } span.sep { "›" }
                    span.cur { "Конфликты" }
                    @if let Some(p) = prop {
                        span.sep { "›" } span.cur { (words::prop(p)) }
                    }
                },
                Some(html! { span.chip.hot[!snap.conflicts.is_empty()] {
                    (number(snap.conflicts.len() as i64)) " всего"
                } }),
            ))
            .wrap {
                @if let Some(item) = settled {
                    .done { span { "Решение записано для " code { (item) } " — граф пересобран." } }
                }

                h1 { "Источники не согласны" }
                p.why {
                    "Сборка уже выбрала значение по приоритету источников. Здесь видно, "
                    "что сказал каждый. Любое решение — «Подтвердить» то, что выбрано, "
                    "«Принять» другой источник или записать своё — убирает расхождение "
                    "из этого списка и переживает пересборку."
                }

                .stats {
                    (stat("все", snap.conflicts.len(), "", Some("/conflicts")))
                    @for (name, count) in &by_prop {
                        (stat(words::prop(name), *count, "warn",
                              Some(&format!("/conflicts?prop={name}"))))
                    }
                }

                (list::controls("/conflicts", q, &hidden, "путь или значение…"))

                @if page.is_empty() {
                    .empty { "Расхождений нет." }
                } @else {
                    p.note {
                        (page.total) " "
                        (words::plural(page.total, "предмет", "предмета", "предметов"))
                        " с расхождениями."
                    }
                    ul.rows {
                        @for (path, found) in &page.rows { (one(snap, path, found)) }
                    }
                }
                (list::pager("/conflicts", q, &hidden, &page))
            }
        },
    )
}

fn one(snap: &Snapshot, path: &str, found: &[&Conflict]) -> Markup {
    html! {
        li.row {
            (heading(snap, path))
            @for c in found { (compare(c)) }
        }
    }
}

/// Who the item is, so a name conflict can be judged without opening it.
fn heading(snap: &Snapshot, path: &str) -> Markup {
    let item = match snap.graph.get(path) {
        Some(Node::Item(item)) => Some(item),
        _ => None,
    };
    html! {
        .mini {
            .mini-shot { img src={ "/icon?q=" (encode(path)) } alt="" loading="lazy"; }
            .grow {
                @match item {
                    Some(item) => {
                        a.row-t href={ "/entity?q=" (encode(path)) } { (title(item)) }
                        @if item.names.ru.is_some() {
                            span.dim { " · " (item.names.en.value) }
                        }
                        (tags(snap, item))
                    }
                    None => a.row-t href={ "/entity?q=" (encode(path)) } { (path) },
                }
                .path { (path) }
            }
        }
    }
}

/// The Russian name where there is one, since that is what the app shows.
fn title(item: &Item) -> &str {
    item.names
        .ru
        .as_ref()
        .map_or(item.names.en.value.as_str(), |r| r.value.as_str())
}

fn tags(snap: &Snapshot, item: &Item) -> Markup {
    html! {
        .tags {
            span.tag.kind { (item.category.value) }
            @if item.prime.value { span.tag.prime { "Prime" } }
            @match &item.tradable {
                Some(t) if t.value => span.tag.trade { "торгуется" },
                Some(_) => span.tag { "не торгуется" },
                None => span.tag { "торгуемость неизвестна" },
            }
            @if let Some(d) = item.ducats { span.tag { (number(d)) " дук." } }
            @if let Some(slug) = &item.slug { span.tag { (slug.value) } }
            @if snap.scope.reason(&item.unique_name).is_some() {
                span.tag.bad { "не в поставке" }
            }
        }
    }
}

/// One property, every claim beside the others so the difference reads in one line.
fn compare(c: &Conflict) -> Markup {
    html! {
        .cmp-h {
            span.cmp-prop { (words::prop(c.prop)) }
            span.dim { "выбрано: " } span.cmp-kept { (c.chosen) }
        }
        .cmp {
            @for (source, value) in &c.claims {
                @let kept = *value == c.chosen;
                .col.kept[kept] {
                    .col-h {
                        (prov(*source)) " " span.dim { (words::source_full(*source)) }
                        @if kept { span.tag.trade { "сейчас" } }
                    }
                    .col-v { (mark(value, &others(c, *source))) }
                    form.inline action="/pick" method="post" {
                        input type="hidden" name="item" value=(c.entity);
                        input type="hidden" name="prop" value=(c.prop);
                        input type="hidden" name="value" value=(value);
                        button type="submit" {
                            @if kept { "Подтвердить" } @else { "Принять" }
                        }
                    }
                }
            }
            @if editable(c.prop) {
                .col {
                    .col-h { span.dim { "свой вариант" } }
                    form.inline action="/pick" method="post" {
                        input type="hidden" name="item" value=(c.entity);
                        input type="hidden" name="prop" value=(c.prop);
                        input type="text" name="value" value=(c.chosen);
                        button.plain type="submit" { "Записать" }
                    }
                }
            }
        }
    }
}

/// What the other sources said, to compare one claim against.
fn others(c: &Conflict, source: Source) -> String {
    c.claims
        .iter()
        .find(|(s, _)| *s != source)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// A claim with the part the other spelling does not share picked out, so the difference is
/// visible without reading both strings end to end. Values that share little — `true`
/// against `false` — are simply different, and marking a fragment of them points at
/// nothing, so they are left alone.
fn mark(value: &str, other: &str) -> Markup {
    let a: Vec<char> = value.chars().collect();
    let b: Vec<char> = other.chars().collect();
    let head = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let shorter = a.len().min(b.len());
    let tail = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
        .min(shorter - head);

    if head + tail < shorter.div_ceil(2) {
        return html! { (value) };
    }

    let take = |from: usize, to: usize| a[from..to].iter().collect::<String>();
    let end = a.len() - tail;
    html! {
        (take(0, head))
        @if head < end { span.diff { (take(head, end)) } }
        (take(end, a.len()))
    }
}

/// Properties whose value a person may write rather than pick: the booleans only have the
/// two values the sources already offer.
fn editable(prop: &str) -> bool {
    matches!(prop, "name_en" | "name_ru")
}

/// Conflicts of one item together, in path order.
fn group<'a>(snap: &'a Snapshot, prop: Option<&str>) -> Vec<(&'a str, Vec<&'a Conflict>)> {
    let mut out: BTreeMap<&str, Vec<&Conflict>> = BTreeMap::new();
    for c in snap
        .conflicts
        .iter()
        .filter(|c| prop.is_none_or(|p| c.prop == p))
    {
        out.entry(c.entity.as_str()).or_default().push(c);
    }
    out.into_iter().collect()
}

/// How many conflicts each property accounts for.
fn tally(snap: &Snapshot) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for c in &snap.conflicts {
        *counts.entry(c.prop).or_default() += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(m: Markup) -> String {
        m.into_string()
    }

    #[test]
    fn a_trailing_word_is_the_only_thing_marked() {
        let out = text(mark("Smeeta Kavat Imprint", "Smeeta Kavat"));
        assert_eq!(out, "Smeeta Kavat<span class=\"diff\"> Imprint</span>");
    }

    #[test]
    fn a_shared_spelling_marks_nothing() {
        assert_eq!(text(mark("Volt Prime", "Volt Prime")), "Volt Prime");
    }

    #[test]
    fn cyrillic_is_split_on_characters() {
        let out = text(mark("Кават: Смита: Отпечаток", "Кават: Смита"));
        assert!(out.starts_with("Кават: Смита<span"));
    }

    #[test]
    fn booleans_share_too_little_to_mark() {
        assert_eq!(text(mark("false", "true")), "false");
        assert_eq!(text(mark("true", "false")), "true");
    }
}
