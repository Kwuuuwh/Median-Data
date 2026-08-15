use std::collections::BTreeMap;

use funnel::{Finding, Layer};
use maud::{Markup, html};
use serde::Deserialize;

use crate::list::{self, Query};
use crate::page::{Side, bar, encode, named, number, shell, slug};
use crate::state::Snapshot;
use crate::words;

/// Which checks the screen is showing.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub rule: String,
}

impl Filter {
    fn hidden(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        if !self.layer.is_empty() {
            out.push(("layer", self.layer.as_str()));
        }
        if !self.rule.is_empty() {
            out.push(("rule", self.rule.as_str()));
        }
        out
    }

    fn keeps(&self, f: &Finding) -> bool {
        (self.layer.is_empty() || f.layer.as_str() == self.layer)
            && (self.rule.is_empty() || f.rule == self.rule)
    }
}

/// What every check found, grouped the way the funnel is built, with one decision per row:
/// accept it, with the reason. An accepted finding stops being asked about and stays counted,
/// so a list looked through once does not come back next build.
pub fn render(snap: &Snapshot, filter: &Filter, q: &Query) -> Markup {
    let rows: Vec<&Finding> = snap
        .report
        .findings
        .iter()
        .filter(|f| filter.keeps(f))
        .filter(|f| q.matches(&format!("{} {} {}", f.entity, f.rule, f.detail)))
        .collect();
    let page = q.page(rows);
    let hidden = filter.hidden();

    shell(
        "Аномалии",
        &Side::of(snap, "anomalies"),
        html! {
            (bar(
                html! {
                    a href="/" { "каталог" } span.sep { "›" }
                    a href="/anomalies" { "Аномалии" }
                    @if !filter.layer.is_empty() {
                        span.sep { "›" } span.cur { (words::layer(&filter.layer)) }
                    }
                    @if !filter.rule.is_empty() {
                        span.sep { "›" } span.cur { (words::rule(&filter.rule)) }
                    }
                },
                Some(html! { span.chip.hot[!snap.report.findings.is_empty()] {
                    (number(snap.report.findings.len() as i64)) " находок"
                } }),
            ))
            .wrap {
                h1 { "Что нашла воронка" }
                p.why {
                    "Инварианты останавливают сборку, остальные слои только сообщают. "
                    "«Принять» — это решение о том, что проверка права, а данные такие и есть; "
                    "оно ложится в " code { "config/curation.toml" } " вместе с причиной."
                }

                (layers(snap, filter))
                (rules(snap, filter))
                (list::controls("/anomalies", q, &hidden, "предмет, правило, подробность…"))

                @if page.is_empty() {
                    .empty { "Ни одна проверка ничего не нашла." }
                } @else {
                    ul.rows {
                        @for f in &page.rows { (row(snap, f)) }
                    }
                }
                (list::pager("/anomalies", q, &hidden, &page))

                h2 id="done" { "Принято" }
                (accepted(snap))
            }
        },
    )
}

/// One finding with the reason it fired and the way to accept it.
pub fn row(snap: &Snapshot, f: &Finding) -> Markup {
    html! {
        li.row id=(row_id(&f.rule, &f.entity)) {
            .row-h {
                span.eyebrow { (words::rule(&f.rule)) }
                span {
                    @if f.layer == Layer::Invariant { span.tag.bad { "инвариант" } " " }
                    span.tag.kind { (words::layer(f.layer.as_str())) }
                }
            }
            .opt {
                .grow.subject {
                    @if snap.graph.has(&f.entity) {
                        (named(&snap.graph, &f.entity))
                    } @else {
                        span.row-t { (f.entity) }
                    }
                    .path { (f.entity) }
                    (about(snap, f))
                    p.note { (f.detail) }
                }
                form.inline hx-post="/accept" hx-target="closest li" hx-swap="outerHTML"
                     action="/accept" method="post" {
                    input type="hidden" name="rule" value=(f.rule);
                    input type="hidden" name="entity" value=(f.entity);
                    input type="text" name="note" placeholder="почему это не дефект";
                    button type="submit" { "Принять" }
                }
            }
        }
    }
}

/// The entities the check names besides the subject, by name rather than by path.
fn about(snap: &Snapshot, f: &Finding) -> Markup {
    html! {
        @if !f.about.is_empty() {
            .opts {
                @for id in &f.about {
                    .opt {
                        .grow {
                            @if snap.graph.has(id) {
                                (named(&snap.graph, id))
                            } @else {
                                span.row-t { (id) }
                            }
                            .path { (id) }
                        }
                    }
                }
            }
        }
    }
}

/// The row a finding turns into once it is accepted.
pub fn taken(rule: &str, entity: &str, note: &str) -> Markup {
    html! {
        li.row.done id=(row_id(rule, entity)) {
            .row-h {
                span.eyebrow { (words::rule(rule)) }
                span.tag.trade { "принято" }
            }
            .subject { span.row-t { (short(entity)) } }
            .path { (entity) }
            .opt {
                .grow { p.note { @if note.is_empty() { "без пояснения" } @else { (note) } } }
                form.inline action="/unaccept" method="post" {
                    input type="hidden" name="rule" value=(rule);
                    input type="hidden" name="entity" value=(entity);
                    button.plain type="submit" { "Вернуть" }
                }
            }
        }
    }
}

/// Every layer with how much it found.
fn layers(snap: &Snapshot, filter: &Filter) -> Markup {
    let counts = snap.report.by_layer();
    html! {
        .chips {
            a class=@if filter.layer.is_empty() && filter.rule.is_empty() { "pick on" } @else { "pick" }
              href="/anomalies" {
                "все " span.num { (number(snap.report.findings.len() as i64)) }
            }
            @for (layer, n) in &counts {
                a class=@if filter.layer == *layer { "pick on" } @else { "pick" }
                  href={ "/anomalies?layer=" (encode(layer)) } {
                    (words::layer(layer)) " " span.num { (number(*n as i64)) }
                }
            }
        }
    }
}

/// Every check of the chosen layer, so a whole class can be worked through at once.
fn rules(snap: &Snapshot, filter: &Filter) -> Markup {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &snap.report.findings {
        if filter.layer.is_empty() || f.layer.as_str() == filter.layer {
            *counts.entry(f.rule.as_str()).or_default() += 1;
        }
    }
    html! {
        @if counts.len() > 1 {
            .chips.sub {
                @for (rule, n) in &counts {
                    @let url = match filter.layer.is_empty() {
                        true => format!("/anomalies?rule={}", encode(rule)),
                        false => format!("/anomalies?layer={}&rule={}",
                                         encode(&filter.layer), encode(rule)),
                    };
                    a class=@if filter.rule == *rule { "pick on" } @else { "pick" } href=(url) {
                        (words::rule(rule)) " " span.num { (number(*n as i64)) }
                    }
                }
            }
        }
    }
}

/// What was accepted, with what was written about it.
fn accepted(snap: &Snapshot) -> Markup {
    html! {
        @if snap.report.accepted.is_empty() {
            .empty { "Ничего не принято — каждая находка ещё ждёт решения." }
        } @else {
            p.why {
                (number(snap.report.accepted.len() as i64)) " "
                (words::plural(snap.report.accepted.len(), "находка принята",
                               "находки приняты", "находок принято"))
                ". Они не пропали: проверки по-прежнему их находят, но больше не спрашивают."
            }
            .scroll { table {
                thead { tr { th { "Проверка" } th { "Что" } th { "Подробность" } th {} } }
                tbody {
                    @for f in &snap.report.accepted {
                        tr {
                            td { (words::rule(&f.rule)) br; span.path { (f.rule) } }
                            td { (crate::page::link(&f.entity, short(&f.entity))) }
                            td.dim { (f.detail) }
                            td.num {
                                form.inline action="/unaccept" method="post" {
                                    input type="hidden" name="rule" value=(f.rule);
                                    input type="hidden" name="entity" value=(f.entity);
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

fn short(entity: &str) -> &str {
    entity.rsplit('/').next().unwrap_or(entity)
}

/// A row's element id, stable for one check and one entity.
pub fn row_id(rule: &str, entity: &str) -> String {
    format!("f-{}-{}", slug(rule), slug(entity))
}
