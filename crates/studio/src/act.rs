use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, html};
use serde::Deserialize;

use crate::page::encode;
use crate::serve::Shared;
use crate::{anomalies, catalog, localize, mapping, patch};

#[derive(Deserialize)]
pub struct MapForm {
    source: String,
    key: String,
    item: String,
}

#[derive(Deserialize)]
pub struct KeyForm {
    source: String,
    key: String,
}

#[derive(Deserialize)]
pub struct DismissForm {
    source: String,
    key: String,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
pub struct NameForm {
    item: String,
    #[serde(default)]
    ru: String,
    /// Which list asked, so only the row a person edited is sent back.
    #[serde(default)]
    frag: String,
    #[serde(default)]
    show: String,
}

#[derive(Deserialize)]
pub struct VerbatimForm {
    kind: String,
    key: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    show: String,
}

#[derive(Deserialize)]
pub struct TermForm {
    kind: String,
    key: String,
    #[serde(default)]
    ru: String,
    /// Which of the two lists asked, so the row comes back reading the same way.
    #[serde(default)]
    show: String,
}

#[derive(Deserialize)]
pub struct PickForm {
    item: String,
    prop: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    back: String,
}

#[derive(Deserialize)]
pub struct AcceptForm {
    rule: String,
    entity: String,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
pub struct Suggest {
    source: String,
    key: String,
    #[serde(default)]
    q: String,
}

/// The items a printed name might mean, as a person types.
pub async fn suggest(State(studio): State<Shared>, Query(ask): Query<Suggest>) -> Markup {
    mapping::candidates(
        &studio.read(),
        &studio.names(),
        &ask.source,
        &ask.key,
        &ask.q,
    )
}

pub async fn map(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<MapForm>,
) -> Response {
    if let Err(e) = studio.store.map(&form.source, &form.key, &form.item) {
        return failed("не удалось сохранить связь", e);
    }
    studio.patch(|snap| patch::mapped(snap, &form.source, &form.key, &form.item));
    if htmx(&headers) {
        return mapping::settled(&studio.read(), &form.source, &form.key, &form.item)
            .into_response();
    }
    Redirect::to("/mapping").into_response()
}

pub async fn unmap(State(studio): State<Shared>, Form(form): Form<KeyForm>) -> Response {
    if let Err(e) = studio.store.unmap(&form.source, &form.key) {
        return failed("не удалось снять связь", e);
    }
    studio.patch(|snap| patch::unmapped(snap, &form.source, &form.key));
    Redirect::to("/mapping").into_response()
}

pub async fn dismiss(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<DismissForm>,
) -> Response {
    if let Err(e) = studio.store.dismiss(&form.source, &form.key, &form.note) {
        return failed("не удалось записать решение", e);
    }
    studio.patch(|snap| patch::dismissed(snap, &form.source, &form.key, &form.note));
    if htmx(&headers) {
        return mapping::dropped(&form.source, &form.key, &form.note).into_response();
    }
    Redirect::to("/mapping").into_response()
}

pub async fn undismiss(State(studio): State<Shared>, Form(form): Form<KeyForm>) -> Response {
    if let Err(e) = studio.store.undismiss(&form.source, &form.key) {
        return failed("не удалось вернуть имя в очередь", e);
    }
    studio.patch(|snap| patch::undismissed(snap, &form.source, &form.key));
    Redirect::to("/mapping").into_response()
}

pub async fn name(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<NameForm>,
) -> Response {
    if let Err(e) = studio.store.name(&form.item, &form.ru) {
        return failed("не удалось записать имя", e);
    }
    studio.patch(|snap| patch::named(snap, &form.item, &form.ru));
    if htmx(&headers) {
        let snap = studio.read();
        return match form.frag.as_str() {
            "catalog" => match catalog::item(&snap, &form.item) {
                Some(item) => catalog::row(&snap, item).into_response(),
                None => html! { tr { td { "предмета нет в снимке: нужна пересборка" } } }
                    .into_response(),
            },
            _ => localize::row_of(&snap, "item", &form.item, &form.show).into_response(),
        };
    }
    Redirect::to(&format!("/entity?q={}", encode(&form.item))).into_response()
}

pub async fn term(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<TermForm>,
) -> Response {
    if let Err(e) = studio.store.term(&form.kind, &form.key, &form.ru) {
        return failed("не удалось записать слово", e);
    }
    studio.patch(|snap| patch::termed(snap, &form.kind, &form.key, &form.ru));
    if htmx(&headers) {
        return localize::row_of(&studio.read(), &form.kind, &form.key, &form.show).into_response();
    }
    Redirect::to(&back_to(&form.kind, &form.show)).into_response()
}

/// Judge that a name stays as the game writes it, or ask for a Russian one again.
pub async fn verbatim(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<VerbatimForm>,
) -> Response {
    if let Err(e) = studio.store.verbatim(&form.kind, &form.key, &form.note) {
        return failed("не удалось записать решение", e);
    }
    studio.patch(|snap| patch::verbatim(snap, &form.kind, &form.key, &form.note, true));
    if htmx(&headers) {
        return localize::row_of(&studio.read(), &form.kind, &form.key, &form.show).into_response();
    }
    Redirect::to(&back_to(&form.kind, &form.show)).into_response()
}

pub async fn unverbatim(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<VerbatimForm>,
) -> Response {
    if let Err(e) = studio.store.unverbatim(&form.kind, &form.key) {
        return failed("не удалось снять решение", e);
    }
    studio.patch(|snap| patch::verbatim(snap, &form.kind, &form.key, "", false));
    if htmx(&headers) {
        return localize::row_of(&studio.read(), &form.kind, &form.key, &form.show).into_response();
    }
    Redirect::to(&back_to(&form.kind, &form.show)).into_response()
}

/// The list a decision was made from.
fn back_to(kind: &str, show: &str) -> String {
    match show.is_empty() {
        true => format!("/localize?what={}", encode(kind)),
        false => format!("/localize?what={}&show={}", encode(kind), encode(show)),
    }
}

pub async fn pick(State(studio): State<Shared>, Form(form): Form<PickForm>) -> Response {
    if let Err(e) = studio.store.pick(&form.item, &form.prop, &form.value) {
        return failed("не удалось записать решение", e);
    }
    studio.patch(|snap| patch::picked(snap, &form.item, &form.prop, &form.value));
    // A pick from a conflict card returns to the list; one from an entity returns to it.
    let back = match form.back.as_str() {
        "entity" => format!("/entity?q={}", encode(&form.item)),
        _ => format!("/conflicts?picked={}", encode(&form.item)),
    };
    Redirect::to(&back).into_response()
}

pub async fn accept(
    State(studio): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<AcceptForm>,
) -> Response {
    if let Err(e) = studio.store.accept(&form.rule, &form.entity, &form.note) {
        return failed("не удалось принять находку", e);
    }
    studio.patch(|snap| patch::accepted(snap, &form.rule, &form.entity));
    if htmx(&headers) {
        return anomalies::taken(&form.rule, &form.entity, &form.note).into_response();
    }
    Redirect::to("/anomalies").into_response()
}

pub async fn unaccept(State(studio): State<Shared>, Form(form): Form<AcceptForm>) -> Response {
    if let Err(e) = studio.store.unaccept(&form.rule, &form.entity) {
        return failed("не удалось вернуть находку", e);
    }
    studio.patch(|snap| patch::unaccepted(snap, &form.rule, &form.entity));
    Redirect::to("/anomalies").into_response()
}

/// Run every source and every check over the decisions again.
pub async fn rebuild(State(studio): State<Shared>) -> Response {
    match studio.rebuild() {
        Ok(()) => Redirect::to("/").into_response(),
        Err(e) => failed("пересборка не удалась", e),
    }
}

fn htmx(headers: &HeaderMap) -> bool {
    headers.contains_key("hx-request")
}

fn failed(what: &str, e: anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(format!(
            "<!doctype html><html lang=ru><meta charset=utf-8>\
             <body style='font:14px sans-serif;background:#0b0f14;color:#e7eef6;padding:24px'>\
             <p>{what}: {e:#}</p><p><a style='color:#00ed64' href='/'>назад</a></p></body></html>"
        )),
    )
        .into_response()
}
