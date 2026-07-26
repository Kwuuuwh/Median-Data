use consensus::Source;
use maud::{DOCTYPE, Markup, html};

use crate::state::Snapshot;

/// The sidebar's work counts, so every screen shows what is left to do.
pub struct Side {
    pub active: &'static str,
    pub mapping: usize,
    pub conflicts: usize,
    pub localize: usize,
    pub anomalies: usize,
    pub stale: bool,
}

impl Side {
    pub fn of(snap: &Snapshot, active: &'static str) -> Self {
        Self {
            active,
            mapping: snap.unresolved.len(),
            conflicts: snap.conflicts.len(),
            localize: crate::localize::pending(snap),
            anomalies: snap.report.findings.len(),
            stale: snap.stale,
        }
    }
}

/// The shell every screen renders into.
pub fn shell(title: &str, side: &Side, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="ru" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — Studio" }
                link rel="stylesheet" href="/style.css";
                script src="/htmx.js" defer {}
            }
            body {
                nav.side {
                    .side-head { span.dot {} "каталог median" }
                    form action="/entity" method="get" {
                        input type="search" name="q" placeholder="найти предмет…";
                    }
                    .side-sec { "разделы" }
                    ul {
                        (item(side, "home", "/", "Обзор", None))
                        (item(side, "catalog", "/catalog", "Каталог", None))
                        (item(side, "localize", "/localize", "Локализация", Some(side.localize)))
                        (item(side, "mapping", "/mapping", "Маппинг", Some(side.mapping)))
                        (item(side, "conflicts", "/conflicts", "Конфликты", Some(side.conflicts)))
                        (item(side, "anomalies", "/anomalies", "Аномалии", Some(side.anomalies)))
                    }
                    (rebuild(side.stale))
                }
                main.main { (body) }
            }
        }
    }
}

/// The way back to a graph that matches the decisions. Decisions show up at once because they
/// are applied to the snapshot in memory; only a rebuild runs every source and every check
/// over them again.
fn rebuild(stale: bool) -> Markup {
    html! {
        .side-sec { "сборка" }
        form.side-build action="/rebuild" method="post" {
            button class=@if stale { "warn" } @else { "plain" } type="submit" {
                @if stale { "Пересобрать" } @else { "Пересобрать заново" }
            }
        }
        @if stale {
            p.side-note { "Решения применены на лету. Пересборка прогонит по ним все источники и проверки." }
        }
    }
}

fn item(side: &Side, name: &str, href: &str, label: &str, count: Option<usize>) -> Markup {
    html! {
        li {
            a href=(href) class=@if side.active == name { "on" } @else { "" } {
                span { (label) }
                @if let Some(n) = count {
                    span.badge.hot[n > 0] { (number(n as i64)) }
                }
            }
        }
    }
}

/// The sticky header: where you are, and one number that matters here.
pub fn bar(crumb: Markup, chip: Option<Markup>) -> Markup {
    html! {
        .bar {
            .crumb { (crumb) }
            @if let Some(chip) = chip { (chip) }
        }
    }
}

/// A labelled number. `tone` is "", "ok", "bad" or "warn".
pub fn stat(label: &str, value: impl std::fmt::Display, tone: &str, href: Option<&str>) -> Markup {
    let class = format!("stat {tone}");
    html! {
        @match href {
            Some(href) => a class=(class) href=(href) {
                span.n { (value) } span.l { (label) }
            },
            None => div class=(class) { span.n { (value) } span.l { (label) } },
        }
    }
}

/// A titled section with an optional count in its corner.
pub fn card(title: &str, count: Option<Markup>, body: Markup) -> Markup {
    html! {
        section.card {
            .card-h {
                span.card-t { (title) }
                @if let Some(count) = count { (count) }
            }
            (body)
        }
    }
}

/// A short badge naming where a claim came from.
pub fn prov(source: Source) -> Markup {
    let class = format!("prov {}", source.as_str());
    html! { span class=(class) { (crate::words::source(source)) } }
}

/// A link to an entity, showing its name over its path.
pub fn link(id: &str, label: &str) -> Markup {
    html! {
        a href={ "/entity?q=" (encode(id)) } { (label) }
    }
}

/// A reference to a node: its picture, its Russian name, and the English one under it.
pub fn named(graph: &graph::Graph, id: &str) -> Markup {
    let (primary, secondary) = names(graph, id);
    html! {
        a.iref href={ "/entity?q=" (encode(id)) } {
            img.iref-icon src={ "/icon?q=" (encode(id)) "&lang=ru" } alt="" loading="lazy";
            span.iref-text {
                span.iref-ru { (primary) }
                @if let Some(en) = secondary { span.iref-en { (en) } }
            }
        }
    }
}

/// A link to a node, its Russian name over the English one. Places and star-chart nodes have
/// no artwork of their own, so this is `named` without the picture.
pub fn plain(graph: &graph::Graph, id: &str) -> Markup {
    let (ru, en) = match names(graph, id) {
        (primary, Some(en)) => (Some(primary), en),
        (en, None) => (None, en),
    };
    html! {
        a href={ "/entity?q=" (encode(id)) } { (dual(ru, en)) }
    }
}

/// A value in Russian with the English original under it, dimmed. With no Russian the English
/// stands alone rather than being doubled.
pub fn dual(ru: Option<&str>, en: &str) -> Markup {
    html! {
        span.bi {
            span.bi-ru { (ru.unwrap_or(en)) }
            @if ru.is_some() { span.bi-en { (en) } }
        }
    }
}

/// A column heading in Russian, with the source's own wording under it.
pub fn col(ru: &str, en: &str) -> Markup {
    html! { th { (ru) span.col-en { (en) } } }
}

/// The Russian name over the English one, or the English name alone when there is no Russian.
pub fn names<'a>(graph: &'a graph::Graph, id: &'a str) -> (&'a str, Option<&'a str>) {
    match graph.get(id) {
        Some(node) => match node.label_ru() {
            Some(ru) => (ru, Some(node.label())),
            None => (node.label(), None),
        },
        None => (id.rsplit('/').next().unwrap_or(id), None),
    }
}

/// A value made safe for an element id: letters and digits kept, everything else a dash.
pub fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Percent-encode a value for a query string.
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A percentage with two decimals.
pub fn pct(chance: f64) -> String {
    format!("{:.2}%", chance * 100.0)
}

/// A quantity that is not whole: kept to as many decimals as it takes to stay a number rather
/// than a row of zeroes.
pub fn amount(value: f64) -> String {
    match value {
        v if v >= 100.0 => number(v.round() as i64),
        v if v >= 1.0 => format!("{v:.2}"),
        v if v >= 0.01 => format!("{v:.3}"),
        v => format!("{v:.4}"),
    }
}

/// Group digits for readability: 15000 -> "15 000".
pub fn number(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push('\u{202F}');
        }
        out.push(*b as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_survive_a_query_string() {
        assert_eq!(encode("/Lotus/Types/A B"), "%2FLotus%2FTypes%2FA%20B");
    }

    #[test]
    fn numbers_group_by_threes() {
        assert_eq!(number(15000), "15\u{202F}000");
        assert_eq!(number(999), "999");
    }
}
