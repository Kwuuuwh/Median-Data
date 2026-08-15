use graph::Graph;
use maud::{Markup, html};

use crate::fold::Fold;
use crate::page::{amount, card, col, number, pct, plain};

use super::drops::{EnemyRow, count, range};

/// The enemies that carry the item. An enemy rolls its drop table first and the row inside it
/// second, so the chance of actually seeing the item is the two multiplied.
pub fn render(graph: &Graph, rows: &[EnemyRow<'_>]) -> Markup {
    if rows.is_empty() {
        return html! {};
    }
    let ranges = rows.iter().any(|r| r.drop.levels.is_some());
    let fold = Fold::new(rows.len());

    card(
        "Выбивается из врагов",
        Some(html! { span.card-n { (number(rows.len() as i64)) } }),
        fold.wrap(html! {
            .scroll { table {
                thead { tr {
                    (col("Враг", "enemy"))
                    @if ranges { (col("Уровень", "level")) }
                    (col("Шанс таблицы", "drop table chance"))
                    (col("Шанс в таблице", "item chance"))
                    (col("Итоговый шанс", "chance"))
                    (col("Убийств", "expected kills"))
                    (col("Кол-во", "quantity"))
                    (col("За убийство", "avg. per roll attempt"))
                } }
                tbody {
                    @for (at, row) in rows.iter().enumerate() {
                        tr.folded[fold.hides(at)] {
                            td { (plain(graph, row.enemy)) }
                            @if ranges { td.num { (range(row.drop.levels)) } }
                            td.num {
                                @match row.drop.table_chance {
                                    Some(chance) => (pct(chance)),
                                    None => "—",
                                }
                            }
                            td.num { (pct(row.drop.chance)) }
                            td.num { (pct(row.drop.total())) }
                            td.num { (kills(row.drop.total())) }
                            td.num { (count(row.drop)) }
                            td.num { (amount(row.drop.per_roll())) }
                        }
                    }
                }
            } }
        }),
    )
}

/// How many kills it takes on average for the item to drop once.
fn kills(chance: f64) -> String {
    match chance > 0.0 {
        true => number((1.0 / chance).ceil() as i64),
        false => "—".to_string(),
    }
}
