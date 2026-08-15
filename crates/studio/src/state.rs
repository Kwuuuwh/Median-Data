use anyhow::Result;
use consensus::{Conflict, Source};
use funnel::Report;
use graph::{Graph, Taxonomy};
use projections::Scope;

/// A name one source prints that the catalog could not tie to an item.
pub struct Unresolved {
    /// Which source printed it: the market, the drop tables, a vendor, the dojo.
    pub source: String,
    /// What a decision about it is keyed by: a market slug, or the printed name.
    pub key: String,
    pub name: String,
    /// Where it was printed, or what the source points at instead.
    pub hint: String,
    /// How many rows hang on this name.
    pub count: usize,
}

/// An item's picture as the build pinned it, for one language.
pub struct Icon {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    /// Whether any pixel is transparent, which the app needs for tiles.
    pub alpha: bool,
    /// Where the picture came from: DE's export or the market's rendered card.
    pub from: Source,
}

/// The decisions on record, so a person can see and take back what they chose.
#[derive(Default)]
pub struct Decided {
    /// Source, the key it names an item by, and the item it was tied to.
    pub links: Vec<(String, String, String)>,
    /// Item path and the Russian name written for it.
    pub names: Vec<(String, String)>,
    /// Item path, property, and the value kept.
    pub picks: Vec<(String, String, String)>,
    /// What kind of thing was named, its key, and the Russian word written for it.
    pub terms: Vec<(String, String, String)>,
    /// Source, the name it prints, and what that name really is — for names no item can
    /// answer to.
    pub dismissed: Vec<(String, String, String)>,
    /// What kind of thing was judged to stay English, its key, and why.
    pub verbatim: Vec<(String, String, String)>,
}

impl Decided {
    pub fn len(&self) -> usize {
        self.links.len()
            + self.names.len()
            + self.picks.len()
            + self.terms.len()
            + self.dismissed.len()
            + self.verbatim.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The Russian word written for one thing, when there is one.
    pub fn term(&self, kind: &str, key: &str) -> Option<&str> {
        self.terms
            .iter()
            .find(|(k, id, _)| k == kind && id == key)
            .map(|(_, _, ru)| ru.as_str())
    }
}

/// Everything Studio shows, as of the last build.
pub struct Snapshot {
    pub graph: Graph,
    /// The taxonomy the build classified items with, for labels and grouping.
    pub taxonomy: Taxonomy,
    pub report: Report,
    pub scope: Scope,
    pub conflicts: Vec<Conflict>,
    /// Every printed name no item answers to, from every source that prints names.
    pub unresolved: Vec<Unresolved>,
    /// Shipped items whose source serves no picture.
    pub iconless: Vec<String>,
    pub decided: Decided,
    /// Whether decisions have been applied to this snapshot in memory since it was built.
    /// A patched snapshot shows the decision at once; only a rebuild makes the whole graph
    /// agree with it.
    pub stale: bool,
}

impl Snapshot {
    /// Unresolved names of one source.
    pub fn from_source<'a>(&'a self, source: &str) -> impl Iterator<Item = &'a Unresolved> {
        let source = source.to_string();
        self.unresolved.iter().filter(move |u| u.source == source)
    }
}

/// What Studio needs the build to do for it. Studio itself only reads and renders.
pub trait Store: Send + Sync {
    /// Reassemble everything from the vault, picking up curated decisions.
    fn rebuild(&self) -> Result<Snapshot>;
    /// Tie a name one source prints to the catalog item it means.
    fn map(&self, source: &str, key: &str, item: &str) -> Result<()>;
    /// Forget a mapping, letting the build derive it again.
    fn unmap(&self, source: &str, key: &str) -> Result<()>;
    /// Record that a printed name names no item at all, with what it really is.
    fn dismiss(&self, source: &str, key: &str, note: &str) -> Result<()>;
    /// Ask about a printed name again.
    fn undismiss(&self, source: &str, key: &str) -> Result<()>;
    /// Write a Russian name by hand. An empty name clears it.
    fn name(&self, item: &str, ru: &str) -> Result<()>;
    /// Keep one source's value for a property a person judged.
    fn pick(&self, item: &str, prop: &str, value: &str) -> Result<()>;
    /// Write the Russian word for something that is not an item.
    fn term(&self, kind: &str, key: &str, ru: &str) -> Result<()>;
    /// Record that a name stays as the game writes it, with why.
    fn verbatim(&self, kind: &str, key: &str, note: &str) -> Result<()>;
    /// Ask for a Russian name again.
    fn unverbatim(&self, kind: &str, key: &str) -> Result<()>;
    /// Let a finding through, with the reason it is not a defect.
    fn accept(&self, rule: &str, entity: &str, note: &str) -> Result<()>;
    /// Report a finding again.
    fn unaccept(&self, rule: &str, entity: &str) -> Result<()>;
    /// The item's pinned picture in one language, when the vault holds it.
    fn icon(&self, item: &str, lang: &str) -> Option<Icon>;
}
