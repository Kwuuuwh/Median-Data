use maud::{Markup, html};
use serde::Deserialize;

use crate::list::{self, Query};
use crate::page::{Side, bar, encode, number, shell, slug};
use crate::state::{Seller, Snapshot};

/// Which vendors the screen is showing.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Filter {
    /// Empty for everyone, `unnamed` for those without a Russian name, `facelesss` for those
    /// without a picture.
    #[serde(default)]
    pub show: String,
}

impl Filter {
    fn keeps(&self, seller: &Seller) -> bool {
        match self.show.as_str() {
            UNNAMED => seller.name_ru.is_none(),
            FACELESS => seller.waiting(),
            _ => true,
        }
    }

    fn hidden(&self) -> Vec<(&str, &str)> {
        match self.show.is_empty() {
            true => Vec::new(),
            false => vec![("show", self.show.as_str())],
        }
    }
}

const UNNAMED: &str = "unnamed";
const FACELESS: &str = "faceless";

/// How many vendors still want a Russian name or a page to be pictured by.
pub fn pending(snap: &Snapshot) -> usize {
    snap.sellers
        .iter()
        .filter(|seller| seller.name_ru.is_none())
        .count()
}

/// Everyone who sells something, and the two things a person has to say about them: what
/// they are called in Russian, and which wiki page pictures them. Neither follows from the
/// game's manifests — a manifest names a counter, not a person. Only the name is counted as
/// work: a vendor without a portrait still tells you what they sell and for how much.
pub fn render(snap: &Snapshot, filter: &Filter, q: &Query) -> Markup {
    let rows: Vec<&Seller> = snap
        .sellers
        .iter()
        .filter(|seller| filter.keeps(seller))
        .filter(|seller| {
            q.matches(&format!(
                "{} {} {}",
                seller.name,
                seller.key,
                seller.name_ru.as_deref().unwrap_or_default()
            ))
        })
        .collect();
    let page = q.page(rows);
    let hidden = filter.hidden();
    let unnamed = snap
        .sellers
        .iter()
        .filter(|seller| seller.name_ru.is_none())
        .count();
    let faceless = snap.sellers.iter().filter(|s| s.waiting()).count();

    shell(
        "Торговцы",
        &Side::of(snap, "sellers"),
        html! {
            (bar(
                html! { a href="/" { "каталог" } span.sep { "›" } span.cur { "Торговцы" } },
                Some(html! { span.chip.hot[pending(snap) > 0] {
                    (number(pending(snap) as i64)) " без имени или портрета"
                } }),
            ))
            .wrap {
                h1 { "Торговцы" }
                p.why {
                    "Прилавки берутся из манифестов игры, а манифест называет счётчик, "
                    "не человека. Русское имя и страница вики, с которой берётся портрет, "
                    "пишутся здесь руками — больше их взять неоткуда."
                }

                .chips {
                    a class=@if filter.show.is_empty() { "pick on" } @else { "pick" } href="/sellers" {
                        "все " span.num { (number(snap.sellers.len() as i64)) }
                    }
                    a class=@if filter.show == UNNAMED { "pick on" } @else { "pick" }
                      href={ "/sellers?show=" (UNNAMED) } {
                        "без имени " span.num { (number(unnamed as i64)) }
                    }
                    a class=@if filter.show == FACELESS { "pick on" } @else { "pick" }
                      href={ "/sellers?show=" (FACELESS) } {
                        "без портрета " span.num { (number(faceless as i64)) }
                    }
                }
                (list::controls("/sellers", q, &hidden, "имя, ключ или перевод…"))

                @if page.is_empty() {
                    .empty { "Никого." }
                } @else {
                    ul.rows {
                        @for seller in &page.rows { (row(seller, &filter.show)) }
                    }
                }
                (list::pager("/sellers", q, &hidden, &page))
            }
        },
    )
}

/// One vendor, with the two fields that decide how the catalog shows them.
pub fn row(seller: &Seller, show: &str) -> Markup {
    let id = format!("seller-{}", slug(&seller.key));
    html! {
        li.row id=(id) {
            .row-h {
                span.row-t { (seller.name) }
                span {
                    @if let Some(area) = &seller.area {
                        span.tag.kind { (area) } " "
                    }
                    span.tag { (number(seller.offers as i64)) " предл." }
                    @if let Some(currency) = &seller.currency {
                        " " span.tag.kind { (currency) }
                    }
                    @if seller.waiting() {
                        " " span.tag.warn { "без портрета" }
                    } @else if !seller.pictured {
                        " " span.tag { "портрет со следующей сборкой" }
                    }
                }
            }
            .path { (seller.key) }
            .curate-row {
                form.inline hx-post="/term" hx-target={ "#" (id) } hx-swap="outerHTML"
                     action="/term" method="post" {
                    input type="hidden" name="kind" value="vendor";
                    input type="hidden" name="key" value=(seller.key);
                    input type="hidden" name="show" value=(show);
                    input type="hidden" name="screen" value="sellers";
                    input type="text" name="ru" value=[seller.name_ru.as_deref()]
                          placeholder="имя по-русски";
                    button.plain type="submit" { "Назвать" }
                }
            }
            .curate-row {
                form.inline hx-post="/portrait" hx-target={ "#" (id) } hx-swap="outerHTML"
                     action="/portrait" method="post" {
                    input type="hidden" name="vendor" value=(seller.key);
                    input type="hidden" name="show" value=(show);
                    input type="text" name="page" value=[seller.page.as_deref()]
                          placeholder="страница wiki.warframe.com";
                    button.plain type="submit" { "Привязать" }
                }
                a.note href={ "https://wiki.warframe.com/index.php?search="
                              (encode(&seller.name)) } target="_blank" { "искать на вики" }
            }
        }
    }
}

/// The row as it stands after a decision, for the swap htmx does in place.
pub fn row_of(snap: &Snapshot, vendor: &str, show: &str) -> Markup {
    match snap.sellers.iter().find(|seller| seller.key == vendor) {
        Some(seller) => row(seller, show),
        None => html! {},
    }
}
