use graph::{Graph, Node, Rel};
use maud::{Markup, html};

use crate::fold::Fold;
use crate::page::{card, col, encode, named, names, number};
use crate::words;

/// How the item is made, what it goes into, and the set it belongs to.
pub fn render(graph: &Graph, id: &str) -> Markup {
    html! {
        (built_here(graph, id))
        (needed_for(graph, id))
        (sets(graph, id))
        (variants(graph, id))
        (wearers(graph, id))
    }
}

/// The two bodies one piece of the operator's wardrobe is worn by. DE models the Drifter as a
/// separate entity under the same name, so both halves are shown together rather than reading
/// as a duplicate.
fn wearers(graph: &Graph, id: &str) -> Markup {
    let drifter: Vec<&str> = graph
        .from(id)
        .into_iter()
        .filter(|e| e.rel == Rel::Fits)
        .map(|e| e.to.as_str())
        .collect();
    let operator: Vec<&str> = graph
        .into(id)
        .into_iter()
        .filter(|e| e.rel == Rel::Fits)
        .map(|e| e.from.as_str())
        .collect();
    if drifter.is_empty() && operator.is_empty() {
        return html! {};
    }

    card(
        "Оператор и Скиталец",
        None,
        html! {
            .strip {
                @for plain in &operator { (tile(graph, plain, None, false)) }
                (tile(graph, id, None, true))
                @for grown in &drifter { (tile(graph, grown, None, false)) }
            }
            p.note {
                @if drifter.is_empty() {
                    "Это версия для Скитальца. Слева — та же вещь для оператора: "
                    "DE держит их как два предмета под одним именем, выдаются они вместе."
                } @else {
                    "Это версия для оператора. Справа — та же вещь для Скитальца: "
                    "DE держит их как два предмета под одним именем, выдаются они вместе."
                }
            }
        },
    )
}

/// The recipes that produce this item, each as a strip of what goes in.
fn built_here(graph: &Graph, id: &str) -> Markup {
    let recipes: Vec<&str> = graph
        .into(id)
        .into_iter()
        .filter(|e| e.rel == Rel::Produces)
        .map(|e| e.from.as_str())
        .collect();

    html! {
        @for recipe in recipes {
            @if let Some(Node::Recipe(r)) = graph.get(recipe) {
                (card("Крафт", Some(html! { span.card-n {
                    @if r.consumed { "чертёж расходуется" } @else { "многоразовый" }
                } }), html! {
                    .strip {
                        (tile(graph, &r.blueprint, None, false))
                        @for e in graph.from(recipe) {
                            @if let Rel::Requires { count } = e.rel {
                                (tile(graph, &e.to, (count > 1).then_some(count), false))
                            }
                        }
                        span.arrow { "→" }
                        (tile(graph, id, None, true))
                    }
                    .meta {
                        span { "Кредиты" b { (r.build_price.map_or("—".into(), number)) } }
                        span { "Время" b { (words::duration(r.build_time.unwrap_or(0))) } }
                        @if let Some(rush) = r.rush_price {
                            span { "Ускорить" b { (number(rush)) " пл." } }
                        }
                    }
                }))
            }
        }
    }
}

/// The recipes that consume this item. A base material feeds hundreds of them, so the list is
/// folded down to what fits on a screen.
fn needed_for(graph: &Graph, id: &str) -> Markup {
    let mut uses: Vec<(&str, i64)> = graph
        .into(id)
        .into_iter()
        .filter_map(|e| match e.rel {
            Rel::Requires { count } => Some((produces(graph, &e.from).unwrap_or(&e.from), count)),
            _ => None,
        })
        .collect();
    if uses.is_empty() {
        return html! {};
    }
    uses.sort_by(|(a, _), (b, _)| names(graph, a).0.cmp(names(graph, b).0));
    let fold = Fold::new(uses.len());

    card(
        "Нужен для",
        Some(html! { span.card-n { (number(uses.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Что собирают", "used for"))
                    (col("Сколько", "quantity"))
                } }
                tbody {
                    @for (at, (result, count)) in uses.iter().enumerate() {
                        tr.folded[fold.hides(at)] {
                            td { (named(graph, result)) }
                            td.num { "×" (number(*count)) }
                        }
                    }
                }
            } }
        }),
    )
}

/// The trade sets this item belongs to, and everything else in them.
fn sets(graph: &Graph, id: &str) -> Markup {
    let owners: Vec<&str> = graph
        .into(id)
        .into_iter()
        .filter(|e| e.rel == Rel::Member)
        .map(|e| e.from.as_str())
        .collect();

    html! {
        @for set in owners {
            @if let Some(Node::Set(s)) = graph.get(set) {
                @let members: Vec<&str> = graph.from(set).into_iter()
                    .filter(|e| e.rel == Rel::Member).map(|e| e.to.as_str()).collect();
                (card("Входит в набор", Some(html! { span.card-n {
                    (members.len()) " " (words::plural(members.len(), "часть", "части", "частей"))
                } }), html! {
                    .row-h {
                        a.row-t href={ "/entity?q=" (encode(set)) } { (s.names.en.value) }
                        @if let Some(d) = s.ducats { span.tag { (number(d)) " дук." } }
                    }
                    .strip {
                        @for member in &members {
                            (tile(graph, member, None, *member == id))
                        }
                        @if let Some(built) = represents(graph, set) {
                            span.arrow { "→" }
                            (tile(graph, built, None, false))
                        }
                    }
                }))
            }
        }
    }
}

/// The plain and prime halves of one weapon or warframe.
fn variants(graph: &Graph, id: &str) -> Markup {
    let up: Vec<&str> = graph
        .from(id)
        .into_iter()
        .filter(|e| e.rel == Rel::Primed)
        .map(|e| e.to.as_str())
        .collect();
    let down: Vec<&str> = graph
        .into(id)
        .into_iter()
        .filter(|e| e.rel == Rel::Primed)
        .map(|e| e.from.as_str())
        .collect();

    html! {
        @if !up.is_empty() || !down.is_empty() {
            (card("Версии", None, html! {
                .strip {
                    @for plain in &down { (tile(graph, plain, None, false)) }
                    (tile(graph, id, None, true))
                    @for prime in &up { (tile(graph, prime, None, false)) }
                }
                p.note {
                    @if !up.is_empty() { "Справа — прайм-версия." }
                    @else { "Слева — обычная версия." }
                }
            }))
        }
    }
}

/// One square in a strip: the artwork, an optional count, and a link.
pub fn tile(graph: &Graph, id: &str, count: Option<i64>, current: bool) -> Markup {
    let name = label(graph, id);
    html! {
        a.ctile.out[current] href={ "/entity?q=" (encode(id)) } title=(name) {
            .ctile-box {
                img src={ "/icon?q=" (encode(id)) } alt="" loading="lazy";
            }
            @match count {
                Some(c) => span.c { "× " (number(c)) },
                None => span.r { (name) },
            }
        }
    }
}

/// The item a recipe produces.
fn produces<'a>(graph: &'a Graph, recipe: &str) -> Option<&'a str> {
    graph
        .from(recipe)
        .into_iter()
        .find(|e| e.rel == Rel::Produces)
        .map(|e| e.to.as_str())
}

/// The assembled item a set stands for.
fn represents<'a>(graph: &'a Graph, set: &str) -> Option<&'a str> {
    graph
        .from(set)
        .into_iter()
        .find(|e| e.rel == Rel::Represents)
        .map(|e| e.to.as_str())
}

/// An entity's display name, falling back to the last path segment.
pub fn label<'a>(graph: &'a Graph, id: &'a str) -> &'a str {
    match graph.get(id) {
        Some(node) => node.label(),
        None => id.rsplit('/').next().unwrap_or(id),
    }
}
