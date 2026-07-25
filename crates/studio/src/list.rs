use maud::{Markup, html};
use serde::Deserialize;

/// How many rows a person may ask for at once. Zero means every one of them: no list in
/// Studio has a cap the caller cannot lift.
pub const SIZES: [usize; 5] = [25, 50, 100, 200, 0];

const DEFAULT: usize = 50;

/// Where a list starts, how long it is, and what it was filtered by. Every screen reads the
/// same query so the controls behave the same everywhere.
#[derive(Debug, Clone, Deserialize)]
pub struct Query {
    /// Free text the rows are filtered by.
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub at: usize,
    #[serde(default)]
    pub limit: Option<usize>,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            q: String::new(),
            at: 0,
            limit: None,
        }
    }
}

impl Query {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT)
    }

    /// Whether a row's text matches the filter. An empty filter matches everything.
    pub fn matches(&self, text: &str) -> bool {
        let needle = self.q.trim().to_lowercase();
        needle.is_empty() || text.to_lowercase().contains(&needle)
    }

    /// The slice of rows this page shows, and the controls to move through the rest.
    pub fn page<T>(&self, rows: Vec<T>) -> Page<T> {
        let total = rows.len();
        let limit = self.limit();
        let at = if self.at >= total { 0 } else { self.at };
        let rows = match limit {
            0 => rows.into_iter().skip(at).collect(),
            n => rows.into_iter().skip(at).take(n).collect(),
        };
        Page {
            rows,
            at,
            limit,
            total,
        }
    }
}

/// One page of rows with what it took to get there.
pub struct Page<T> {
    pub rows: Vec<T>,
    pub at: usize,
    pub limit: usize,
    pub total: usize,
}

impl<T> Page<T> {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    fn end(&self) -> usize {
        match self.limit {
            0 => self.total,
            n => (self.at + n).min(self.total),
        }
    }
}

/// The filter box and the row-count picker, as one form. `hidden` carries whatever else the
/// screen is keyed by, so filtering never loses the tab a person is on.
pub fn controls(action: &str, q: &Query, hidden: &[(&str, &str)], placeholder: &str) -> Markup {
    html! {
        form.filter action=(action) method="get" {
            @for (name, value) in hidden {
                input type="hidden" name=(name) value=(value);
            }
            input type="search" name="q" value=(q.q) placeholder=(placeholder);
            select name="limit" onchange="this.form.submit()" {
                @for size in SIZES {
                    option value=(size) selected[size == q.limit()] {
                        @if size == 0 { "все" } @else { (size) " строк" }
                    }
                }
            }
            button type="submit" { "Показать" }
            @if !q.q.trim().is_empty() {
                a.plain href=(url(action, hidden, "", q.limit(), 0)) { "сбросить" }
            }
        }
    }
}

/// Step controls: where in the list this page sits, and the way back and forward.
pub fn pager<T>(action: &str, q: &Query, hidden: &[(&str, &str)], page: &Page<T>) -> Markup {
    if page.at == 0 && page.end() >= page.total {
        return html! {};
    }
    let step = |at: usize| url(action, hidden, &q.q, page.limit, at);
    let back = page.at.saturating_sub(page.limit.max(1));
    html! {
        .pager {
            @if page.at > 0 {
                a href=(step(back)) { "← предыдущие" }
            } @else {
                span.off { "← предыдущие" }
            }
            span.range { (page.at + 1) "–" (page.end()) " из " (page.total) }
            @if page.end() < page.total {
                a href=(step(page.at + page.limit.max(1))) { "следующие →" }
            } @else {
                span.off { "следующие →" }
            }
        }
    }
}

/// A link to the same screen with the list controls set.
pub fn url(action: &str, hidden: &[(&str, &str)], q: &str, limit: usize, at: usize) -> String {
    let mut out = format!("{action}?limit={limit}&at={at}");
    if !q.trim().is_empty() {
        out.push_str(&format!("&q={}", crate::page::encode(q)));
    }
    for (name, value) in hidden {
        out.push_str(&format!("&{name}={}", crate::page::encode(value)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(at: usize, limit: usize) -> Query {
        Query {
            q: String::new(),
            at,
            limit: Some(limit),
        }
    }

    #[test]
    fn a_page_is_a_window_on_the_rows() {
        let page = q(2, 2).page((1..=5).collect());
        assert_eq!(page.rows, vec![3, 4]);
        assert_eq!(page.total, 5);
    }

    #[test]
    fn no_limit_means_every_row() {
        let page = q(0, 0).page((1..=5).collect());
        assert_eq!(page.rows.len(), 5);
    }

    #[test]
    fn an_offset_past_the_end_falls_back_to_the_top() {
        let page = q(40, 10).page((1..=5).collect());
        assert_eq!(page.at, 0);
        assert_eq!(page.rows.len(), 5);
    }

    #[test]
    fn the_filter_is_case_blind_and_matches_anywhere() {
        let mut query = Query::default();
        query.q = "prime".into();
        assert!(query.matches("Volt Prime Chassis"));
        assert!(!query.matches("Volt Chassis"));
        assert!(Query::default().matches("anything"));
    }
}
