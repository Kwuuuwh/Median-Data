use std::path::Path;

use anyhow::Result;
use consensus::Conflict;
use funnel::Report;
use graph::{Graph, Taxonomy};
use rusqlite::Transaction;

use crate::icons;
use crate::scope::Scope;

/// Everything a projection may read. The graph is the single source; a projection never
/// reaches back to a fetcher or a parser.
pub struct Context<'a> {
    pub graph: &'a Graph,
    pub taxonomy: &'a Taxonomy,
    pub scope: &'a Scope,
    pub report: &'a Report,
    pub conflicts: &'a [Conflict],
    /// Directory the file artifacts go to.
    pub out: &'a Path,
    /// Pinned icon bytes, when a run has fetched them.
    pub icons: Option<&'a dyn icons::Source>,
}

/// What a projection produced, for the build summary.
pub struct Summary {
    pub name: &'static str,
    pub detail: String,
}

/// One view of the graph, rendered into an artifact.
pub trait Projection {
    fn name(&self) -> &'static str;

    /// Write into the catalog database. Projections that produce a file instead leave this
    /// alone and use `file`.
    fn db(&self, _tx: &Transaction<'_>, _ctx: &Context<'_>) -> Result<Option<Summary>> {
        Ok(None)
    }

    /// Write a standalone file artifact.
    fn file(&self, _ctx: &Context<'_>) -> Result<Option<Summary>> {
        Ok(None)
    }
}
