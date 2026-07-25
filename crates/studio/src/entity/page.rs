use consensus::Conflict;
use funnel::Finding;
use graph::{Graph, Node};
use maud::{Markup, html};

use crate::page::{Side, bar, card, encode, shell};
use crate::state::{Snapshot, Store};
use crate::words;

use super::{craft, drops, facts, head, stock, web};

/// How many search hits are worth listing.
const HITS: usize = 60;

/// Everything known about one entity, and every entity it touches.
pub fn render(snap: &Snapshot, store: &dyn Store, query: &str) -> Markup {
    let query = query.trim();
    if query.is_empty() {
        return page(
            snap,
            "Поиск",
            html! { span.cur { "Поиск" } },
            html! { .empty { "Введите название или путь в строке слева." } },
        );
    }

    if let Some(node) = snap.graph.get(query) {
        return one(snap, store, node);
    }
    let hits = search(&snap.graph, query);
    match hits.as_slice() {
        [] => page(
            snap,
            query,
            html! { span.cur { (query) } },
            html! { .empty { "Ничего не найдено по запросу «" (query) "»." } },
        ),
        [only] => one(snap, store, only),
        many => {
            let word = words::plural(many.len(), "совпадение", "совпадения", "совпадений");
            page(
                snap,
                query,
                html! {
                    a href="/" { "каталог" } span.sep { "›" }
                    span.cur { (many.len()) " " (word) }
                },
                html! {
                    h1 { "«" (query) "»" }
                    .scroll { table {
                        thead { tr { th { "Название" } th { "Путь" } } }
                        tbody {
                            @for node in many {
                                @let id = node.id();
                                tr {
                                    td { a href={ "/entity?q=" (encode(&id)) } { (node.label()) } }
                                    td.path { (id) }
                                }
                            }
                        }
                    } }
                },
            )
        }
    }
}

fn one(snap: &Snapshot, store: &dyn Store, node: &Node) -> Markup {
    let id = node.id();
    let (ru, en) = (store.icon(&id, "ru"), store.icon(&id, "en"));
    let found: Vec<&Finding> = snap
        .report
        .findings
        .iter()
        .filter(|f| f.entity == id)
        .collect();
    let clashes: Vec<&Conflict> = snap.conflicts.iter().filter(|c| c.entity == id).collect();

    page(
        snap,
        node.label(),
        html! {
            a href="/" { "каталог" } span.sep { "›" }
            @if let Node::Item(item) = node {
                a href={ "/entity?q=" (encode(&item.category.value)) } { (item.category.value) }
                span.sep { "›" }
            }
            span.cur { (node.label()) }
        },
        html! {
            (head::render(snap, node, ru.as_ref(), en.as_ref()))
            (problems(&found))
            (web::render(&snap.graph, &id))
            @if let Node::Item(item) = node { (facts::render(item, &clashes)) }
            (craft::render(&snap.graph, &id))
            (drops::render(&snap.graph, &id))
            (stock::sold_by(&snap.graph, &id))
            (stock::sold_at(&snap.graph, &id))
            @if let Node::Item(item) = node { (curate(snap, item)) }
            (rest(&snap.graph, &id))
            (facts::legend())
        },
    )
}

/// What the funnel said about this entity, if anything.
fn problems(found: &[&Finding]) -> Markup {
    html! {
        @if !found.is_empty() {
            (card("Проверки не сошлись", Some(html! {
                span.card-n.hot { (found.len()) }
            }), html! {
                .scroll { table {
                    thead { tr { th { "Проверка" } th { "Что именно" } } }
                    tbody {
                        @for f in found {
                            tr {
                                td { (words::rule(&f.rule)) br; span.path { (f.rule) } }
                                td.dim { (f.detail) }
                            }
                        }
                    }
                } }
            }))
        }
    }
}

/// Setting by hand what the rules leave wrong: the Russian name, tradability, and where the
/// item sits in the taxonomy.
fn curate(snap: &Snapshot, item: &graph::Item) -> Markup {
    let path = item.unique_name.as_str();
    let value = item.tradable.as_ref().map(|t| t.value);
    let by_hand = item
        .tradable
        .as_ref()
        .is_some_and(|t| t.sources.contains(&consensus::Source::Curated));
    card(
        "Курация",
        None,
        html! {
            .curate-row {
                span.dim { "Русское имя" }
                form.inline action="/name" method="post" {
                    input type="hidden" name="item" value=(path);
                    input type="text" name="ru" placeholder="русское название";
                    button type="submit" {
                        @if item.names.ru.is_some() { "Заменить" } @else { "Задать" }
                    }
                }
            }
            .curate-row {
                span.dim { "Торгуемость" }
                (flag(path, "true", "торгуется", by_hand && value == Some(true)))
                (flag(path, "false", "не торгуется", by_hand && value == Some(false)))
                (flag(path, "", "по источникам", !by_hand))
            }
            (kind(snap, item))
            p.note {
                "Ручное решение переживает пересборку. «По источникам» снимает его и "
                "возвращает то, что дают DE и рынок."
            }
        },
    )
}

/// Moving an item to another kind. The list is the declared tree, so only a kind the
/// taxonomy knows can be chosen.
fn kind(snap: &Snapshot, item: &graph::Item) -> Markup {
    let path = item.unique_name.as_str();
    let current = item.kind.value.as_str();
    let by_hand = item.kind.sources.contains(&consensus::Source::Curated);
    html! {
        .curate-row {
            span.dim { "Категория" }
            form.inline action="/pick" method="post" {
                input type="hidden" name="item" value=(path);
                input type="hidden" name="prop" value="kind";
                input type="hidden" name="back" value="entity";
                select name="value" {
                    @for class in snap.taxonomy.classes() {
                        optgroup label=(class.ru) {
                            @for leaf in &class.kind {
                                option value=(leaf.slug) selected[leaf.slug == current] {
                                    (leaf.ru)
                                }
                            }
                        }
                    }
                }
                button type="submit" { "Перенести" }
            }
            @if by_hand {
                (flag_kind(path, "", "по правилам"))
            } @else {
                span.dim { "по правилам" }
            }
        }
    }
}

/// Dropping a hand-made kind decision, letting the rules classify again.
fn flag_kind(path: &str, value: &str, label: &str) -> Markup {
    html! {
        form.inline action="/pick" method="post" {
            input type="hidden" name="item" value=(path);
            input type="hidden" name="prop" value="kind";
            input type="hidden" name="value" value=(value);
            input type="hidden" name="back" value="entity";
            button.plain type="submit" { (label) }
        }
    }
}

/// One tradability choice; the active one is highlighted, the rest muted.
fn flag(path: &str, value: &str, label: &str, active: bool) -> Markup {
    html! {
        form.inline action="/pick" method="post" {
            input type="hidden" name="item" value=(path);
            input type="hidden" name="prop" value="tradable";
            input type="hidden" name="value" value=(value);
            input type="hidden" name="back" value="entity";
            button.plain[!active] type="submit" { (label) }
        }
    }
}

/// Edges no dedicated section covered, so nothing is hidden.
fn rest(graph: &Graph, id: &str) -> Markup {
    use graph::Rel;
    let covered = |rel: &Rel| {
        matches!(
            rel,
            Rel::Produces | Rel::Requires { .. } | Rel::Rewards { .. } | Rel::Drops(_)
                | Rel::Member | Rel::Primed | Rel::Sells(_)
        )
    };
    let out: Vec<(&str, &Rel)> = graph
        .from(id)
        .into_iter()
        .filter(|e| !covered(&e.rel))
        .map(|e| (e.to.as_str(), &e.rel))
        .collect();
    let inc: Vec<(&str, &Rel)> = graph
        .into(id)
        .into_iter()
        .filter(|e| !covered(&e.rel))
        .map(|e| (e.from.as_str(), &e.rel))
        .collect();

    html! {
        @if !out.is_empty() || !inc.is_empty() {
            (card("Прочие связи", None, html! {
                .scroll { table {
                    tbody {
                        @for (other, rel) in &out {
                            tr {
                                td.dim { (words::rel(rel)) }
                                td { a href={ "/entity?q=" (encode(other)) }
                                       { (craft::label(graph, other)) } }
                            }
                        }
                        @for (other, rel) in &inc {
                            tr {
                                td.dim { (words::rel_back(rel)) }
                                td { a href={ "/entity?q=" (encode(other)) }
                                       { (craft::label(graph, other)) } }
                            }
                        }
                    }
                } }
            }))
        }
    }
}

fn page(snap: &Snapshot, title: &str, crumb: Markup, body: Markup) -> Markup {
    shell(
        title,
        &Side::of(snap, "entity"),
        html! { (bar(crumb, None)) .wrap { (body) } },
    )
}

/// Nodes whose id or name contains the query, shortest name first.
fn search<'a>(graph: &'a Graph, query: &str) -> Vec<&'a Node> {
    let needle = query.to_lowercase();
    let mut hits: Vec<&Node> = graph
        .nodes()
        .filter(|n| {
            n.label().to_lowercase().contains(&needle) || n.id().to_lowercase().contains(&needle)
        })
        .collect();
    hits.sort_by_key(|n| (n.label().len(), n.label().to_string()));
    hits.truncate(HITS);
    hits
}
