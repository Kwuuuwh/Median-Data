use graph::{Extra, Item, Node};
use maud::{Markup, html};

use crate::page::{card, encode, number, prov};
use crate::state::{Icon, Snapshot};
use crate::words;

/// Who the entity is: its picture, both names, and every flag at a glance.
pub fn render(snap: &Snapshot, node: &Node, ru: Option<&Icon>, en: Option<&Icon>) -> Markup {
    let id = node.id();
    html! {
        .head {
            (shot(&id, "ru", ru))
            .grow {
                @match node {
                    Node::Item(item) => (item_head(snap, item)),
                    Node::Set(set) => {
                        h1 { (set.names.en.value) }
                        @if let Some(name) = &set.names.ru { p.ru { (name.value) } }
                        .tags {
                            span.tag.kind { "набор рынка" }
                            @if let Some(d) = set.ducats { span.tag { (number(d)) " дук." } }
                        }
                    }
                    Node::Imprint(imprint) => {
                        h1 { (imprint.names.en.value) }
                        @if let Some(name) = &imprint.names.ru { p.ru { (name.value) } }
                        .tags {
                            span.tag.kind { "отпечаток" }
                            span.tag.trade { "торгуется" }
                        }
                    }
                    Node::Recipe(r) => {
                        h1 { "Рецепт" }
                        .tags {
                            @if r.consumed {
                                span.tag { "чертёж расходуется" }
                            } @else {
                                span.tag { "чертёж многоразовый" }
                            }
                        }
                    }
                    Node::Place(p) => {
                        @match &p.table {
                            Some(table) => {
                                h1 { (table.node) }
                                p.ru { (table.location) " · " (table.label) }
                            }
                            None => {
                                h1 { (p.name) }
                                @if let Some(name) = &p.name_ru { p.ru { (name) } }
                            }
                        }
                        @if let Some(b) = &p.bounty {
                            p.ru {
                                (b.activity_ru.as_deref().unwrap_or(&b.activity))
                                " · " (b.settlement_ru.as_deref().unwrap_or(&b.settlement))
                            }
                        }
                        .tags {
                            span.tag.kind { (words::place(p.kind)) }
                            @if let Some(table) = &p.table {
                                @if table.extra { span.tag { "доп. таблица" } }
                                @if table.event { span.tag.bad { "таблица события" } }
                            }
                            @if let Some(b) = &p.bounty {
                                @if b.max_level > 0 {
                                    span.tag { "ур. " (b.min_level) "–" (b.max_level) }
                                }
                                @match b.giver_ru.as_deref().or(b.giver.as_deref()) {
                                    Some(giver) => span.tag { "выдаёт: " (giver) },
                                    None => span.tag.bad { "кто выдаёт — неизвестно" },
                                }
                            }
                        }
                    }
                    Node::Enemy(e) => {
                        h1 { (e.name) }
                        @match &e.name_ru {
                            Some(name) => p.ru { (name) },
                            None => p.ru.none { "русского имени нет" },
                        }
                        .tags { span.tag.kind { "враг" } }
                    }
                    Node::Vendor(v) => {
                        h1 { (v.name) }
                        @if let Some(name) = &v.name_ru { p.ru { (name) } }
                        .tags {
                            span.tag.kind { (words::vendor(v.kind.as_deref())) }
                            @if let Some(currency) = &v.currency {
                                span.tag { "платят: " (currency) }
                            }
                            @if v.rotates { span.tag.bad { "ассортимент меняется" } }
                        }
                    }
                    Node::Lab(l) => {
                        h1 { (l.name) }
                        .tags {
                            span.tag.kind { "лаборатория додзё" }
                            span.tag { (l.faction) }
                        }
                    }
                    Node::Location(l) => {
                        h1 { (l.name) }
                        @match &l.name_ru {
                            Some(name) => p.ru { (name) },
                            None => p.ru.none { "русского имени нет" },
                        }
                        .tags {
                            @match &l.kind {
                                Some(kind) => span.tag.kind { (words::location(kind)) },
                                None => span.tag.bad { "тип не указан" },
                            }
                        }
                    }
                    Node::Region(r) => {
                        @let site = snap.graph.get(&graph::location_id(&r.location));
                        @let where_ = site
                            .and_then(|n| n.label_ru())
                            .unwrap_or(r.location.as_str());
                        h1 { (r.name) }
                        @match &r.name_ru {
                            Some(name) => p.ru { (name) " · " (where_) },
                            None => p.ru.none { (where_) " · русского имени узла нет" },
                        }
                        .tags {
                            (enum_tag(&r.type_label, r.node_type))
                            (enum_tag(&r.mission_label, r.mission))
                            (enum_tag(&r.faction_label, r.faction))
                            @if r.max_level > 0 {
                                span.tag { "ур. " (r.min_level) "–" (r.max_level) }
                            }
                            @if r.mastery_req > 0 { span.tag { "с ранга " (r.mastery_req) } }
                            @if r.mastery_xp > 0 { span.tag { "мастерство " (r.mastery_xp) } }
                            @if r.railjack { span.tag { "рейлджек" } }
                            @if r.hidden { span.tag.bad { "не на карте" } }
                            (prov(r.origin))
                        }
                    }
                }
                .path { (id) }
            }
        }
        (pictures(&id, ru, en))
    }
}

/// One of DE's numbered enums. An index the reference table does not name shows as the bare
/// number, so an unnamed one is visible rather than hidden. A negative index means DE gave
/// no number at all, and there is nothing to show.
fn enum_tag(label: &graph::Label, index: i64) -> Markup {
    html! {
        @if index >= 0 {
            @match label.ru.as_deref().or(label.en.as_deref()) {
                Some(name) => span.tag.kind { (name) },
                None => span.tag.bad { "не расшифровано: " (index) },
            }
        }
    }
}

fn item_head(snap: &Snapshot, item: &Item) -> Markup {
    let held = snap.scope.reason(&item.unique_name);
    html! {
        h1 { (item.names.en.value) }
        @match &item.names.ru {
            Some(ru) => p.ru { (ru.value) },
            None => p.ru.none { "русского имени нет" },
        }
        .tags {
            span.tag.kind { (snap.taxonomy.class_label(&item.kind.value, "ru")) }
            span.tag.kind { (snap.taxonomy.label(&item.kind.value, "ru")) }
            span.tag title=(item.category.value) { "DE: " (item.category.value) }
            @if item.prime.value { span.tag.prime { "Prime" } }
            @match &item.tradable {
                Some(t) if t.value => span.tag.trade { "торгуется" },
                Some(_) => span.tag { "не торгуется" },
                None => span.tag { "торгуемость неизвестна" },
            }
            @if let Some(d) = item.ducats { span.tag { (number(d)) " дук." } }
            @if let Some(slug) = &item.slug { span.tag { (slug.value) } }
            @if let Extra::Relic(r) = &item.extra {
                span.tag { (words::refinement(&r.refinement)) }
            }
            @match item.vaulted.as_ref().map(|v| v.value) {
                Some(true) => span.tag.bad {
                    "в хранилище"
                    @if let Extra::Relic(r) = &item.extra {
                        @if let Some(version) = &r.vaulted_in { " с " (version) }
                    }
                },
                Some(false) => span.tag.trade { "добывается" },
                None => {}
            }
            @match held {
                Some(reason) => span.tag.bad { "не в поставке: " (reason) },
                None => span.tag.trade { "в поставке" },
            }
        }
    }
}

fn shot(id: &str, lang: &str, icon: Option<&Icon>) -> Markup {
    html! {
        .shot {
            @match icon {
                Some(_) => img src={ "/icon?q=" (encode(id)) "&lang=" (lang) } alt="";
                None => span.none { "нет" br; "картинки" },
            }
        }
    }
}

/// What the catalog will actually ship for this item, per language. A mod carries its name
/// and stats in the picture, so the two languages are two different files.
fn pictures(id: &str, ru: Option<&Icon>, en: Option<&Icon>) -> Markup {
    let differ = match (ru, en) {
        (Some(a), Some(b)) => a.bytes != b.bytes,
        _ => false,
    };
    card(
        "Картинка",
        Some(html! {
            @if differ {
                span.card-n { "по одной на язык" }
            } @else {
                span.card-n { "одна на оба языка" }
            }
        }),
        html! {
            @if ru.is_none() && en.is_none() {
                .tags {
                    span.tag.bad { "картинки нет" }
                    span.dim { "сборка не запинила её для этого пути." }
                }
            } @else {
                .shots {
                    (one("русская", id, "ru", ru))
                    @if differ { (one("английская", id, "en", en)) }
                }
            }
        },
    )
}

fn one(label: &str, id: &str, lang: &str, icon: Option<&Icon>) -> Markup {
    html! {
        .pic {
            .pic-box {
                @if icon.is_some() {
                    img src={ "/icon?q=" (encode(id)) "&lang=" (lang) } alt="";
                }
            }
            .pic-facts {
                span.dim { (label) }
                @match icon {
                    None => span.tag.bad { "нет" },
                    Some(i) => {
                        .tags {
                            (prov(i.from)) " "
                            span.tag { (i.width) "×" (i.height) }
                            span.tag { (i.mime) }
                            @if i.alpha {
                                span.tag.trade { "с прозрачностью" }
                            } @else {
                                span.tag.bad { "без прозрачности" }
                            }
                        }
                        span.dim { (words::source_full(i.from)) }
                    }
                }
            }
        }
    }
}
