use maud::{Markup, html};

use crate::page::number;
use crate::words;

/// How many rows of a list are worth reading before it stops being a list and becomes a wall.
const SHOWN: usize = 12;

/// A list long enough to fold: the first rows stay, the rest wait behind a toggle. The whole
/// list is in the markup either way, so the browser's own search still finds every row.
pub struct Fold {
    total: usize,
}

impl Fold {
    pub fn new(total: usize) -> Self {
        Self { total }
    }

    /// Whether the row at this position is one of the folded ones.
    pub fn hides(&self, at: usize) -> bool {
        self.total > SHOWN && at >= SHOWN
    }

    /// The list with its toggle, or the list untouched when it is short enough to read.
    pub fn wrap(&self, body: Markup) -> Markup {
        if self.total <= SHOWN {
            return body;
        }
        let rest = self.total - SHOWN;
        html! {
            .fold {
                (body)
                details {
                    summary {
                        span.fold-more {
                            "показать ещё " (number(rest as i64)) " "
                            (words::plural(rest, "строку", "строки", "строк"))
                        }
                        span.fold-less { "свернуть" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_list_is_left_alone() {
        let fold = Fold::new(SHOWN);
        assert!(!fold.hides(SHOWN - 1));
        assert_eq!(fold.wrap(html! { "rows" }).into_string(), "rows");
    }

    #[test]
    fn a_long_list_keeps_the_first_rows_and_folds_the_rest() {
        let fold = Fold::new(SHOWN + 5);
        assert!(!fold.hides(SHOWN - 1));
        assert!(fold.hides(SHOWN));
        assert!(fold.wrap(html! {}).into_string().contains("показать ещё 5"));
    }
}
