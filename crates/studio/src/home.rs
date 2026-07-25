use std::collections::BTreeMap;

use graph::Node;
use maud::{Markup, html};

use crate::page::{Side, bar, card, link, number, shell, stat};
use crate::state::Snapshot;
use crate::{terms, words};

/// How many examples to name beside a rule's count.
const SHOWN: usize = 3;

/// Everything one build produced, on one screen: what the catalog holds, what wants a
/// person, what every check found, and what moved since last time.
pub fn render(snap: &Snapshot) -> Markup {
    let t = &snap.report.totals;

    shell(
        "Обзор",
        &Side::of(snap, "home"),
        html! {
            (bar(
                html! { span.cur { "Обзор" } },
                Some(html! { span.chip.hot[snap.report.queue() > 0] {
                    (number(snap.report.queue() as i64)) " на ревью"
                } }),
            ))
            .wrap {
                h1 { "Каталог" }
                p.why { "Собран из запиненных источников — любое число ниже воспроизводится." }
                @if snap.stale {
                    .done {
                        span { "Снимок правился на лету: решения видно сразу, но граф ещё не пересобран." }
                        form.inline action="/rebuild" method="post" {
                            button type="submit" { "Пересобрать" }
                        }
                    }
                }
                .stats {
                    (stat("предметов", number(t.items as i64), "", Some("/catalog")))
                    (stat("в поставке", number(snap.scope.in_scope() as i64), "ok", None))
                    (stat("удержано", number(snap.scope.excluded() as i64), "", None))
                    (stat("мест", number(t.places as i64), "", None))
                    (stat("узлов карты", number(regions(snap) as i64), "", None))
                    (stat("связей", number(t.edges as i64), "", None))
                }

                h2 { "Требует человека" }
                .stats {
                    (stat("имён без предмета", snap.unresolved.len(),
                          tone(snap.unresolved.len()), Some("/mapping")))
                    (stat("конфликтов", snap.conflicts.len(),
                          tone(snap.conflicts.len()), Some("/conflicts")))
                    (stat("без русского имени", terms::pending(snap),
                          tone(terms::pending(snap)), Some("/localize")))
                    (stat("находок воронки", snap.report.findings.len(),
                          tone(snap.report.findings.len()), Some("/anomalies")))
                    (stat("принято", snap.report.accepted.len(), "", Some("/anomalies#done")))
                    (stat("решено рукой", snap.decided.len(), "", None))
                }

                h2 { "Откуда берутся предметы" }
                (obtainable(snap))

                h2 { "По категориям" }
                (categories(snap))

                h2 { "Русский язык" }
                (russian(snap))

                h2 { "Что нашла воронка" }
                @if snap.report.findings.is_empty() {
                    .empty { "Ни одна проверка ничего не нашла." }
                } @else {
                    @for (layer, rules) in by_layer(snap) {
                        (card(
                            words::layer(layer),
                            Some(html! { span.card-n { (total(&rules)) } }),
                            html! { .scroll { table {
                                thead { tr {
                                    th { "Проверка" } th { "Штук" } th { "Примеры" }
                                } }
                                tbody {
                                    @for (rule, hits) in &rules {
                                        tr {
                                            td { a href={ "/anomalies?rule=" (crate::page::encode(rule)) } {
                                                     (words::rule(rule)) }
                                                 br; span.path { (rule) } }
                                            td.num { (hits.len()) }
                                            td.dim {
                                                @for entity in hits.iter().take(SHOWN) {
                                                    (link(entity, short(entity))) " "
                                                }
                                                @if hits.len() > SHOWN {
                                                    span.path { "и ещё " (hits.len() - SHOWN) }
                                                }
                                            }
                                        }
                                    }
                                }
                            } } },
                        ))
                    }
                }

                @let tally = snap.scope.tally();
                @if !tally.is_empty() {
                    h2 { "Удержано намеренно" }
                    (card("Правила поставки", None, html! {
                        table {
                            thead { tr { th { "Причина" } th { "Предметов" } } }
                            tbody {
                                @for (reason, n) in tally {
                                    tr { td { (reason) } td.num { (number(n as i64)) } }
                                }
                            }
                        }
                    }))
                }

                @if !snap.iconless.is_empty() {
                    h2 { "Без картинки" }
                    (card("Источник не отдаёт изображение", Some(html! {
                        span.card-n.hot { (snap.iconless.len()) }
                    }), html! {
                        ul.rows {
                            @for path in snap.iconless.iter().take(10) {
                                li.row { (link(path, short(path))) " " span.path { (path) } }
                            }
                        }
                    }))
                }

                h2 { "С прошлой сборки" }
                (diff(snap))
            }
        },
    )
}

/// How much of the catalog we can say where it comes from, and where the holes are. A hole is
/// a source nobody has read yet, not a broken item — so it is a to-do list for sources.
fn obtainable(snap: &Snapshot) -> Markup {
    let mut gaps: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut shipped, mut known, mut untracked) = (0, 0, 0);
    for item in snap.graph.items() {
        if !snap.scope.allows(&item.unique_name) {
            continue;
        }
        if !snap.taxonomy.sourced(&item.kind.value) {
            untracked += 1;
            continue;
        }
        shipped += 1;
        if graph::obtainable(&snap.graph, &item.unique_name) {
            known += 1;
        } else {
            *gaps
                .entry(snap.taxonomy.class_slug(&item.kind.value))
                .or_default() += 1;
        }
    }
    let mut ranked: Vec<(&str, usize)> = gaps.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    html! {
        .stats {
            (stat("источник известен", number(known as i64), "ok", None))
            (stat("без источника", number((shipped - known) as i64), tone(shipped - known),
                  Some("/catalog?only=source")))
            (stat("всего отслеживаем", number(shipped as i64), "", None))
            (stat("не отслеживаем", number(untracked as i64), "", None))
        }
        (card("Чего не хватает, по классам", None, html! {
            table {
                thead { tr { th { "Класс" } th { "Без источника" } } }
                tbody {
                    @for (class, n) in &ranked {
                        tr {
                            td { a href={ "/catalog?class=" (crate::page::encode(class))
                                          "&only=source" } { (class) } }
                            td.num { (number(*n as i64)) }
                        }
                    }
                }
            }
        }))
    }
}

/// Every class of the taxonomy with its kinds, so a wrong classification is visible as a
/// count that looks off.
fn categories(snap: &Snapshot) -> Markup {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for item in snap.graph.items() {
        *counts.entry(item.kind.value.as_str()).or_default() += 1;
    }
    let total = |class: &graph::Class| -> usize {
        class
            .kind
            .iter()
            .map(|leaf| counts.get(leaf.slug.as_str()).copied().unwrap_or(0))
            .sum()
    };
    html! {
        (card("Классы и подкатегории", Some(html! {
            span.card-n { (snap.taxonomy.classes().len()) }
        }), html! {
            .scroll { table {
                thead { tr { th { "Класс" } th { "Штук" } th { "Подкатегории" } } }
                tbody {
                    @for class in snap.taxonomy.classes() {
                        @let n = total(class);
                        tr {
                            td { a href={ "/catalog?class=" (crate::page::encode(&class.slug)) } {
                                     (class.ru) }
                                 br; span.path { (class.slug) } }
                            td.num { (number(n as i64)) }
                            td.dim {
                                @for leaf in &class.kind {
                                    @let k = counts.get(leaf.slug.as_str()).copied().unwrap_or(0);
                                    a.tag.bad[k == 0] href={ "/catalog?class="
                                        (crate::page::encode(&class.slug)) "&kind="
                                        (crate::page::encode(&leaf.slug)) } {
                                        (leaf.ru) " " span.num { (number(k as i64)) }
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

/// What the Russian side actually rests on. A name a rule derived is not a translation, so
/// counting it as covered would be a lie.
fn russian(snap: &Snapshot) -> Markup {
    use consensus::Source;
    let mut by: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut none = 0;
    for item in snap.graph.items() {
        match &item.names.ru {
            Some(ru) => *by.entry(words::source_full(ru.winner)).or_default() += 1,
            None => none += 1,
        }
    }
    let hand = by
        .get(words::source_full(Source::Curated))
        .copied()
        .unwrap_or(0);
    let derived = by
        .get(words::source_full(Source::Rule))
        .copied()
        .unwrap_or(0);
    let (places, no_place_ru) = terms::tally(snap, "place");
    html! {
        .stats {
            (stat("предметов без RU", number(none as i64), tone(none),
                  Some("/localize?what=item")))
            (stat("RU выведено правилом", number(derived as i64), tone(derived), None))
            (stat("RU написано рукой", number(hand as i64), "", None))
            (stat(&format!("мест без RU · всего {}", number(places as i64)),
                  number(no_place_ru as i64),
                  tone(no_place_ru), Some("/localize?what=place")))
        }
    }
}

fn diff(snap: &Snapshot) -> Markup {
    html! {
        @match &snap.report.diff {
            None => .empty { "Сравнивать не с чем — прошлой сборки нет." },
            Some(d) if d.is_empty() => .empty { "Ничего не изменилось." },
            Some(d) => {
                .stats {
                    (stat("предметов пришло", d.items_added.len(), "ok", None))
                    (stat("предметов ушло", d.items_removed.len(),
                          tone(d.items_removed.len()), None))
                    (stat("наборов пришло", d.sets_added.len(), "", None))
                    (stat("наборов ушло", d.sets_removed.len(), "", None))
                }
                @if !d.findings_delta.is_empty() {
                    (card("Находки", None, html! {
                        table {
                            thead { tr { th { "Проверка" } th { "Изменение" } } }
                            tbody {
                                @for (rule, delta) in &d.findings_delta {
                                    tr {
                                        td { (words::rule(rule)) }
                                        td.num {
                                            @if *delta > 0 {
                                                span.tag.bad { "+" (delta) }
                                            } @else {
                                                span.tag.trade { (delta) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }))
                }
                (moved("Пришло", &d.items_added))
                (moved("Ушло", &d.items_removed))
            }
        }
    }
}

fn moved(title: &str, paths: &[String]) -> Markup {
    const LIST: usize = 25;
    html! {
        @if !paths.is_empty() {
            (card(title, Some(html! { span.card-n { (paths.len()) } }), html! {
                ul.rows {
                    @for path in paths.iter().take(LIST) {
                        li.row { (link(path, short(path))) " " span.path { (path) } }
                    }
                }
                @if paths.len() > LIST {
                    p.note { "и ещё " (paths.len() - LIST) " — весь список в " code { "catalog.changes.md" } }
                }
            }))
        }
    }
}

fn regions(snap: &Snapshot) -> usize {
    snap.graph
        .nodes()
        .filter(|n| matches!(n, Node::Region(_)))
        .count()
}

/// Findings grouped by layer, then by rule, keeping the entities each rule named.
fn by_layer(snap: &Snapshot) -> BTreeMap<&'static str, BTreeMap<&str, Vec<&str>>> {
    let mut out: BTreeMap<&'static str, BTreeMap<&str, Vec<&str>>> = BTreeMap::new();
    for f in &snap.report.findings {
        out.entry(f.layer.as_str())
            .or_default()
            .entry(f.rule.as_str())
            .or_default()
            .push(&f.entity);
    }
    out
}

fn total(rules: &BTreeMap<&str, Vec<&str>>) -> usize {
    rules.values().map(Vec::len).sum()
}

fn tone(n: usize) -> &'static str {
    if n > 0 { "bad" } else { "ok" }
}

fn short(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
