use graph::{Graph, Node, Rel};
use maud::{Markup, PreEscaped, html};

use crate::page::encode;

/// Node box size and the gaps around it, in CSS pixels.
const W: f64 = 176.0;
const H: f64 = 54.0;
const GX: f64 = 92.0;
const GY: f64 = 14.0;
const PAD: f64 = 14.0;
/// Most neighbours drawn on one side before the rest fold into a tally.
const CAP: usize = 8;

/// A neighbour to draw: the node it stands for (none for an overflow tally), what to call
/// it, the relation that ties it to the opened item, and whether it carries a picture.
struct GNode {
    id: Option<String>,
    name: String,
    rel: &'static str,
    icon: bool,
}

/// The opened item at the centre and the nodes one hop away: on the left what it comes from
/// (its recipe's inputs, the relics and places that drop it), on the right what it feeds
/// (recipes that need it, its prime or base, the set it belongs to).
pub fn render(graph: &Graph, id: &str) -> Markup {
    let mut left = Vec::new();
    let mut right = Vec::new();

    for e in graph.into(id) {
        match &e.rel {
            Rel::Produces => {
                if let Some(Node::Recipe(r)) = graph.get(&e.from) {
                    push(&mut left, graph, id, &r.blueprint, "чертёж");
                    for ing in graph.from(&e.from) {
                        if let Rel::Requires { .. } = ing.rel {
                            push(&mut left, graph, id, &ing.to, "крафт");
                        }
                    }
                }
            }
            Rel::Rewards { .. } => push(&mut left, graph, id, &e.from, "реликвия"),
            Rel::Drops(_) => push(&mut left, graph, id, &e.from, "дроп"),
            Rel::Yields => push(&mut left, graph, id, &e.from, "отпечаток"),
            Rel::Requires { .. } => {
                if let Some(result) = produces(graph, &e.from) {
                    push(&mut right, graph, id, result, "нужен для");
                }
            }
            Rel::Primed => push(&mut right, graph, id, &e.from, "базовая"),
            Rel::Member | Rel::Represents => push(&mut right, graph, id, &e.from, "сет"),
            Rel::At => push(&mut left, graph, id, &e.from, "таблица наград"),
            Rel::Sells(_) => push(&mut left, graph, id, &e.from, "продаёт"),
            Rel::Refines => push(&mut left, graph, id, &e.from, "улучшение"),
            Rel::Researched(_) => push(&mut left, graph, id, &e.from, "исследование"),
        }
    }
    for e in graph.from(id) {
        match e.rel {
            Rel::Primed => push(&mut right, graph, id, &e.to, "прайм"),
            Rel::At => push(&mut right, graph, id, &e.to, "узел"),
            Rel::Refines => push(&mut right, graph, id, &e.to, "улучшается в"),
            _ => {}
        }
    }

    fold(&mut left);
    fold(&mut right);
    if left.is_empty() && right.is_empty() {
        return html! {};
    }

    let block = |n: usize| {
        if n == 0 {
            0.0
        } else {
            n as f64 * H + (n - 1) as f64 * GY
        }
    };
    let max_block = block(left.len()).max(H).max(block(right.len()));
    let mid = PAD + max_block / 2.0;
    let total_h = max_block + 2.0 * PAD;

    let mut x = PAD;
    let left_x = x;
    if !left.is_empty() {
        x += W + GX;
    }
    let center_x = x;
    x += W;
    let mut right_x = center_x;
    if !right.is_empty() {
        x += GX;
        right_x = x;
        x += W;
    }
    let total_w = x + PAD;

    let col_top = |n: usize| mid - block(n) / 2.0;
    let cy = |n: usize, i: usize| col_top(n) + i as f64 * (H + GY) + H / 2.0;

    let mut paths = String::new();
    for i in 0..left.len() {
        edge(&mut paths, left_x + W, cy(left.len(), i), center_x, mid);
    }
    for j in 0..right.len() {
        edge(&mut paths, center_x + W, mid, right_x, cy(right.len(), j));
    }
    let svg = format!(
        "<svg class='gedges' viewBox='0 0 {total_w:.0} {total_h:.0}' \
         width='{total_w:.0}' height='{total_h:.0}' preserveAspectRatio='xMinYMin meet'>\
         {paths}</svg>"
    );

    crate::page::card(
        "Граф связей",
        None,
        html! {
            .scroll {
                .gwrap style=(format!("position:relative;width:{total_w:.0}px;height:{total_h:.0}px")) {
                    (PreEscaped(svg))
                    @for (i, n) in left.iter().enumerate() { (draw(n, left_x, cy(left.len(), i))) }
                    (centre(graph, id, center_x, mid))
                    @for (j, n) in right.iter().enumerate() { (draw(n, right_x, cy(right.len(), j))) }
                }
            }
            p.note { "Открытый предмет выделен. Любой узел кликабелен — переход к нему." }
        },
    )
}

/// Record a neighbour once, skipping the opened item itself and anything already listed.
fn push(list: &mut Vec<GNode>, graph: &Graph, centre: &str, id: &str, rel: &'static str) {
    if id == centre || list.iter().any(|n| n.id.as_deref() == Some(id)) {
        return;
    }
    list.push(GNode {
        id: Some(id.to_string()),
        name: disp(graph, id).to_string(),
        rel,
        icon: matches!(graph.get(id), Some(Node::Item(_))),
    });
}

/// Trim a side to the cap, folding the overflow into one tally node.
fn fold(list: &mut Vec<GNode>) {
    if list.len() > CAP {
        let extra = list.len() - (CAP - 1);
        list.truncate(CAP - 1);
        list.push(GNode {
            id: None,
            name: format!("ещё {extra}"),
            rel: "",
            icon: false,
        });
    }
}

fn edge(paths: &mut String, x1: f64, y1: f64, x2: f64, y2: f64) {
    let cx = (x1 + x2) / 2.0;
    paths.push_str(&format!(
        "<path class='gedge' d='M{x1:.1} {y1:.1} C{cx:.1} {y1:.1} {cx:.1} {y2:.1} {x2:.1} {y2:.1}'/>"
    ));
}

fn draw(n: &GNode, x: f64, cy: f64) -> Markup {
    let style = box_style(x, cy);
    html! {
        @match &n.id {
            Some(id) => a.gnode href={ "/entity?q=" (encode(id)) } style=(style) title=(n.name) {
                @if n.icon { img.gicon src={ "/icon?q=" (encode(id)) } alt="" loading="lazy"; }
                .gtext {
                    @if !n.rel.is_empty() { span.grel { (n.rel) } }
                    span.gname { (n.name) }
                }
            },
            None => div.gnode.more style=(style) { span.gname { (n.name) } },
        }
    }
}

fn centre(graph: &Graph, id: &str, x: f64, cy: f64) -> Markup {
    let style = box_style(x, cy);
    let name = disp(graph, id);
    html! {
        div.gnode.cur style=(style) title=(name) {
            @if matches!(graph.get(id), Some(Node::Item(_))) {
                img.gicon src={ "/icon?q=" (encode(id)) } alt="" loading="lazy";
            }
            .gtext { span.gname { (name) } }
        }
    }
}

fn box_style(x: f64, cy: f64) -> String {
    format!(
        "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px",
        x,
        cy - H / 2.0,
        W,
        H
    )
}

/// The item a recipe produces.
fn produces<'a>(graph: &'a Graph, recipe: &str) -> Option<&'a str> {
    graph
        .from(recipe)
        .into_iter()
        .find(|e| e.rel == Rel::Produces)
        .map(|e| e.to.as_str())
}

/// A node's Russian display name, falling back to English, then the last path segment.
fn disp<'a>(graph: &'a Graph, id: &'a str) -> &'a str {
    let ru = |n: &'a graph::Names| {
        n.ru.as_ref()
            .map_or(n.en.value.as_str(), |r| r.value.as_str())
    };
    match graph.get(id) {
        Some(Node::Item(i)) => ru(&i.names),
        Some(Node::Set(s)) => ru(&s.names),
        Some(Node::Imprint(i)) => ru(&i.names),
        Some(Node::Place(p)) => &p.name,
        Some(Node::Region(r)) => r.name_ru.as_deref().unwrap_or(&r.name),
        Some(Node::Vendor(v)) => v.name_ru.as_deref().unwrap_or(&v.name),
        Some(Node::Lab(l)) => &l.name,
        Some(Node::Recipe(r)) => &r.blueprint,
        None => id.rsplit('/').next().unwrap_or(id),
    }
}
