use anyhow::Result;
use consensus::{Conflict, Source};
use funnel::Report;
use graph::{Graph, Taxonomy};
use projections::Scope;
use std::collections::{BTreeMap, BTreeSet};

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
    /// When the build first saw this name with nothing to tie it to. Zero means the build
    /// kept no record — the first build after this was added, or a name the screen made up.
    pub since_ms: i64,
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
    /// Which client the distillate came from, beside the one installed here.
    pub client: Client,
    /// The taxonomy the build classified items with, for labels and grouping.
    pub taxonomy: Taxonomy,
    pub report: Report,
    pub scope: Scope,
    pub conflicts: Vec<Conflict>,
    /// When each disagreement was first seen, keyed as `<prop> <entity>`. Sources drift apart
    /// for a few days after a patch and come back together on their own.
    pub conflict_since: BTreeMap<String, i64>,
    /// Every printed name no item answers to, from every source that prints names.
    pub unresolved: Vec<Unresolved>,
    /// Shipped items whose source serves no picture.
    pub iconless: Vec<String>,
    /// Every vendor of the catalog, with the wiki page that pictures them.
    pub sellers: Vec<Seller>,
    /// Names the game client prints the same way in Russian, so no translation is owed.
    pub kept: BTreeSet<String>,
    /// Every phrase the client prints at all, folded to lower case. A name missing from it is
    /// not a name the game shows: it is a heading a source built for itself, and no Russian
    /// for it exists to be found.
    pub spoken: BTreeSet<String>,
    pub decided: Decided,
    /// Whether decisions have been applied to this snapshot in memory since it was built.
    /// A patched snapshot shows the decision at once; only a rebuild makes the whole graph
    /// agree with it.
    pub stale: bool,
}

/// A vendor as the screen that ties them to a page and a Russian name reads them.
#[derive(Debug, Clone)]
pub struct Seller {
    pub key: String,
    pub name: String,
    pub name_ru: Option<String>,
    /// The wiki page tying them to a picture, where one is known.
    pub page: Option<String>,
    /// Whether that page actually yielded a picture the vault holds.
    pub pictured: bool,
    pub offers: usize,
    pub currency: Option<String>,
    /// The hub they stand in, where the game's own directories say which.
    pub area: Option<String>,
}

impl Seller {
    /// Nothing points at a picture for them: no page was named and none was found.
    pub fn waiting(&self) -> bool {
        !self.pictured && self.page.is_none()
    }
}

/// The client a build reads the cache of, as two dates.
#[derive(Debug, Default, Clone)]
pub struct Client {
    /// The client the distillate in the repository was taken from.
    pub distilled: String,
    /// The client installed on this machine, where the cache is at hand.
    pub installed: Option<String>,
}

impl Client {
    /// Whether the machine holds a newer client than the distillate was taken from.
    pub fn behind(&self) -> bool {
        self.installed
            .as_deref()
            .is_some_and(|installed| installed > self.distilled.as_str())
    }
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
    /// Say which wiki page pictures a vendor. An empty page clears it.
    fn portrait(&self, vendor: &str, page: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_cache_than_the_distillate_is_an_update() {
        let taken = |installed: Option<&str>| Client {
            distilled: "2026.09.22".to_string(),
            installed: installed.map(str::to_string),
        };

        assert!(taken(Some("2026.09.25")).behind());
        assert!(!taken(Some("2026.09.22")).behind());
        assert!(!taken(None).behind());
    }
}
