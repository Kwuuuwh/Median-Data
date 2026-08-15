use graph::{Graph, Rel};

/// How an item can be got, as short labels with how many ways of each kind lead to it. An
/// empty list is the honest answer "no source we hold explains this item", which is a to-do
/// for the sources, not a defect of the item.
pub fn of(graph: &Graph, item: &str) -> Vec<(&'static str, usize)> {
    let mut counts = [0usize; 8];
    for edge in graph.into(item) {
        let at = match edge.rel {
            Rel::Drops(_) if edge.from.starts_with("enemy:") => 0,
            Rel::Drops(_) => 1,
            Rel::Rewards { .. } => 2,
            Rel::Sells(_) => 3,
            Rel::Researched(_) => 4,
            Rel::Produces => 5,
            Rel::Member => 6,
            Rel::Refines => 7,
            _ => continue,
        };
        counts[at] += 1;
    }
    const LABELS: [&str; 8] = [
        "враг",
        "дроп",
        "реликвия",
        "торговец",
        "исследование",
        "рецепт",
        "набор",
        "улучшение",
    ];
    LABELS
        .iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(label, n)| (*label, n))
        .collect()
}
