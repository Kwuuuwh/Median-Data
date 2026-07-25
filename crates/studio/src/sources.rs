use graph::{Graph, Rel};

/// How an item can be got, as short labels with how many ways of each kind lead to it. An
/// empty list is the honest answer "no source we hold explains this item", which is a to-do
/// for the sources, not a defect of the item.
pub fn of(graph: &Graph, item: &str) -> Vec<(&'static str, usize)> {
    let mut counts = [0usize; 7];
    for edge in graph.into(item) {
        let at = match edge.rel {
            Rel::Drops(_) => 0,
            Rel::Rewards { .. } => 1,
            Rel::Sells(_) => 2,
            Rel::Researched(_) => 3,
            Rel::Produces => 4,
            Rel::Member => 5,
            Rel::Refines => 6,
            _ => continue,
        };
        counts[at] += 1;
    }
    const LABELS: [&str; 7] = [
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
