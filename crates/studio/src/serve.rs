use std::sync::{Arc, RwLock, RwLockReadGuard};

use anyhow::Result;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use maud::Markup;
use serde::Deserialize;

use crate::css::SHEET;
use crate::fuzzy::Names;
use crate::list;
use crate::state::{Snapshot, Store};
use crate::{act, anomalies, catalog, conflicts, entity, home, localize, mapping};

/// The vendored copy of htmx, served from here so no page ever asks a CDN for anything.
const HTMX: &str = include_str!("../static/htmx.min.js");

/// Stands in for artwork the vault never pinned, so a missing icon leaves an empty square
/// rather than a broken image.
const BLANK: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x00, 0x02, 0x00,
    0x00, 0x05, 0x00, 0x01, 0x7a, 0x5e, 0xab, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

/// One in-memory graph, the name index built over it, and the build that can produce both
/// again.
pub struct Studio {
    snapshot: RwLock<Snapshot>,
    names: RwLock<Names>,
    pub store: Box<dyn Store>,
}

pub type Shared = Arc<Studio>;

impl Studio {
    pub fn read(&self) -> RwLockReadGuard<'_, Snapshot> {
        self.snapshot.read().expect("snapshot lock")
    }

    pub fn names(&self) -> RwLockReadGuard<'_, Names> {
        self.names.read().expect("names lock")
    }

    /// Show a decision at once, without running every source over it again.
    pub fn patch(&self, change: impl FnOnce(&mut Snapshot)) {
        change(&mut self.snapshot.write().expect("snapshot lock"));
    }

    /// Assemble everything again, so the whole graph and every check agree with the decisions.
    pub fn rebuild(&self) -> Result<()> {
        let fresh = self.store.rebuild()?;
        *self.names.write().expect("names lock") = Names::build(&fresh.graph);
        *self.snapshot.write().expect("snapshot lock") = fresh;
        Ok(())
    }
}

/// Serve Studio until interrupted.
pub async fn run(addr: &str, store: Box<dyn Store>) -> Result<()> {
    let snapshot = store.rebuild()?;
    let names = Names::build(&snapshot.graph);
    let studio: Shared = Arc::new(Studio {
        snapshot: RwLock::new(snapshot),
        names: RwLock::new(names),
        store,
    });

    let app = Router::new()
        .route("/", get(overview))
        .route("/catalog", get(shelf))
        .route("/localize", get(translation))
        .route("/mapping", get(names_without_items))
        .route("/conflicts", get(disagreements))
        .route("/anomalies", get(findings))
        .route("/entity", get(one_entity))
        .route("/suggest", get(act::suggest))
        .route("/map", post(act::map))
        .route("/unmap", post(act::unmap))
        .route("/dismiss", post(act::dismiss))
        .route("/undismiss", post(act::undismiss))
        .route("/name", post(act::name))
        .route("/term", post(act::term))
        .route("/pick", post(act::pick))
        .route("/accept", post(act::accept))
        .route("/unaccept", post(act::unaccept))
        .route("/rebuild", post(act::rebuild))
        .route("/icon", get(icon))
        .route("/style.css", get(stylesheet))
        .route("/htmx.js", get(script))
        .with_state(studio);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Studio on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Every screen's list controls, flat: a nested struct cannot be read out of a query string.
#[derive(Debug, Default, Deserialize)]
struct Screen {
    #[serde(default)]
    q: String,
    #[serde(default)]
    at: usize,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    class: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    only: String,
    #[serde(default)]
    what: String,
    #[serde(default)]
    show: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    layer: String,
    #[serde(default)]
    rule: String,
    #[serde(default)]
    prop: Option<String>,
    #[serde(default)]
    picked: String,
}

impl Screen {
    fn list(&self) -> list::Query {
        list::Query {
            q: self.q.clone(),
            at: self.at,
            limit: self.limit,
        }
    }
}

#[derive(Deserialize)]
struct Find {
    #[serde(default)]
    q: String,
}

#[derive(Deserialize)]
struct Picture {
    #[serde(default)]
    q: String,
    /// The catalog ships a picture per language; the UI reads Russian.
    #[serde(default = "russian")]
    lang: String,
}

fn russian() -> String {
    "ru".to_string()
}

async fn overview(State(studio): State<Shared>) -> Markup {
    home::render(&studio.read())
}

async fn shelf(State(studio): State<Shared>, Query(s): Query<Screen>) -> Markup {
    let filter = catalog::Filter {
        class: s.class.clone(),
        kind: s.kind.clone(),
        only: s.only.clone(),
    };
    catalog::render(&studio.read(), &filter, &s.list())
}

async fn translation(State(studio): State<Shared>, Query(s): Query<Screen>) -> Markup {
    let filter = localize::Filter {
        what: s.what.clone(),
        show: s.show.clone(),
    };
    localize::render(&studio.read(), &filter, &s.list())
}

async fn names_without_items(State(studio): State<Shared>, Query(s): Query<Screen>) -> Markup {
    let filter = mapping::Filter {
        source: s.source.clone(),
    };
    mapping::render(&studio.read(), &studio.names(), &filter, &s.list())
}

async fn disagreements(State(studio): State<Shared>, Query(s): Query<Screen>) -> Markup {
    let picked = (!s.picked.is_empty()).then_some(s.picked.clone());
    conflicts::render(
        &studio.read(),
        s.prop.as_deref(),
        picked.as_deref(),
        &s.list(),
    )
}

async fn findings(State(studio): State<Shared>, Query(s): Query<Screen>) -> Markup {
    let filter = anomalies::Filter {
        layer: s.layer.clone(),
        rule: s.rule.clone(),
    };
    anomalies::render(&studio.read(), &filter, &s.list())
}

async fn one_entity(State(studio): State<Shared>, Query(find): Query<Find>) -> Markup {
    entity::render(&studio.read(), studio.store.as_ref(), &find.q)
}

async fn icon(State(studio): State<Shared>, Query(find): Query<Picture>) -> Response {
    match studio.store.icon(&find.q, &find.lang) {
        Some(icon) => ([(header::CONTENT_TYPE, icon.mime)], icon.bytes).into_response(),
        None => ([(header::CONTENT_TYPE, "image/png")], BLANK).into_response(),
    }
}

async fn stylesheet() -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], SHEET).into_response()
}

async fn script() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        HTMX,
    )
        .into_response()
}
