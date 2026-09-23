use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use consensus::Source;
use studio::{Icon, Snapshot, Store};
use vault::{BlobId, Vault};

use crate::curation::Curation;
use crate::{build, curation, extract, icons, spec, unmatched};

/// Serve Studio over the pinned sources. Every screen reads one in-memory graph; a curated
/// decision rewrites the file and reassembles it.
pub fn run(vault_dir: &str, addr: &str, cache: Option<&str>) -> Result<()> {
    let inspector = Inspector::open(vault_dir, cache)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(studio::run(addr, Box::new(inspector)))
}

/// The build, as Studio drives it.
struct Inspector {
    vault_dir: PathBuf,
    scope: PathBuf,
    curation: PathBuf,
    /// Which blob holds each item's picture per language, resolved once at startup — the
    /// vault only changes when `fetch` or `icons` runs, and neither runs from here.
    pictures: icons::Pictures,
    /// The game cache on this machine, where Studio runs beside the client, so a screen can
    /// say that a newer client is installed than the distillate was taken from.
    cache: Option<PathBuf>,
    /// Held for the whole read-change-write of the decisions file. Two people working at
    /// once send their decisions at once, and without this the later write would drop the
    /// earlier one.
    writing: Mutex<()>,
}

/// The hub a vendor stands in, named the way the catalog names it.
fn area_of(graph: &graph::Graph, vendor: &str) -> Option<String> {
    let area = graph
        .from(vendor)
        .into_iter()
        .find(|edge| edge.rel == graph::Rel::Within)?
        .to
        .clone();
    match graph.get(&area) {
        Some(graph::Node::Area(area)) => Some(area.name_ru.clone().unwrap_or(area.name.clone())),
        _ => None,
    }
}

/// Every vendor of the catalog, with the page that pictures them and what they sell.
fn sellers(
    graph: &graph::Graph,
    curated: &Curation,
    pictures: &std::collections::BTreeMap<String, String>,
) -> Vec<studio::Seller> {
    let by_hand = curated.portraits();
    let mut out: Vec<studio::Seller> = graph
        .nodes()
        .filter_map(|node| match node {
            graph::Node::Vendor(vendor) => Some(vendor),
            _ => None,
        })
        .map(|vendor| studio::Seller {
            area: area_of(graph, &graph::vendor_id(&vendor.key)),
            page: by_hand.get(vendor.key.as_str()).map(|p| (*p).to_string()),
            pictured: pictures.contains_key(&vendor.key),
            offers: graph.from(&graph::vendor_id(&vendor.key)).len(),
            key: vendor.key.clone(),
            name: vendor.name.clone(),
            name_ru: vendor.name_ru.clone(),
            currency: vendor.currency.clone(),
        })
        .collect();
    out.sort_by(|one, other| one.name.cmp(&other.name));
    out
}

impl Inspector {
    fn open(vault_dir: &str, cache: Option<&str>) -> Result<Self> {
        let vault = Vault::open(vault_dir)?;
        Ok(Self {
            vault_dir: vault_dir.into(),
            scope: PathBuf::from(crate::SCOPE),
            curation: PathBuf::from(crate::CURATION),
            pictures: pictures(&vault).unwrap_or_default(),
            cache: cache.map(PathBuf::from),
            writing: Mutex::new(()),
        })
    }

    /// Which client the distillate came from, beside the one whose cache is at hand.
    fn client(&self) -> Result<studio::Client> {
        Ok(studio::Client {
            distilled: crate::game::client(Path::new(crate::GAME))?.build,
            installed: self
                .cache
                .as_deref()
                .map(crate::game::installed)
                .transpose()?,
        })
    }

    /// Read the decisions, change them, write them back, one writer at a time.
    fn edit(&self, change: impl FnOnce(&mut Curation)) -> Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let mut curated = curation::load(&self.curation)?;
        change(&mut curated);
        curation::save(&self.curation, &curated)
    }
}

impl Store for Inspector {
    fn rebuild(&self) -> Result<Snapshot> {
        let vault = Vault::open(&self.vault_dir)?;
        let curated = curation::load(&self.curation)?;
        let built = build::graph_with(&vault, &curated)?;
        let (report, state) = build::judge(&vault, &built, &curated)?;

        let policy = projections::load(&self.scope)?;
        let scope = projections::apply(&built.graph, &policy);

        let mut unresolved = built.orphans;
        unresolved.extend(unmatched::find(&built.wfm, &built.matched));
        let dismissed = curated.dismissed();
        unresolved.retain(|u| !dismissed.contains(&(u.source.as_str(), u.key.as_str())));
        for row in &mut unresolved {
            row.since_ms = state
                .waiting
                .get(&row.source)
                .and_then(|keys| keys.get(&row.key))
                .copied()
                .unwrap_or_default();
        }

        let iconless = built
            .graph
            .items()
            .filter(|i| scope.allows(&i.unique_name) && !self.pictures.contains_key(&i.unique_name))
            .map(|i| i.unique_name.clone())
            .collect();

        let spoken = crate::game::spoken(Path::new(crate::GAME))?;
        Ok(Snapshot {
            client: self.client()?,
            spoken: spoken
                .name
                .keys()
                .map(|phrase| phrase.to_lowercase())
                .collect(),
            kept: spoken.verbatim,
            sellers: sellers(&built.graph, &curated, &icons::vendor_pictures(&vault)),
            graph: built.graph,
            taxonomy: built.taxonomy,
            report,
            scope,
            conflict_since: state
                .waiting
                .get(build::CONFLICT)
                .cloned()
                .unwrap_or_default(),
            conflicts: built.conflicts,
            unresolved,
            iconless,
            decided: curated.decided(),
            stale: false,
        })
    }

    fn map(&self, source: &str, key: &str, item: &str) -> Result<()> {
        self.edit(|c| c.set_link(source, key, item))
    }

    fn unmap(&self, source: &str, key: &str) -> Result<()> {
        self.edit(|c| c.clear_link(source, key))
    }

    fn dismiss(&self, source: &str, key: &str, note: &str) -> Result<()> {
        self.edit(|c| c.set_dismiss(source, key, note))
    }

    fn undismiss(&self, source: &str, key: &str) -> Result<()> {
        self.edit(|c| c.clear_dismiss(source, key))
    }

    fn portrait(&self, vendor: &str, page: &str) -> Result<()> {
        self.edit(|c| c.set_portrait(vendor, page))
    }

    fn name(&self, item: &str, ru: &str) -> Result<()> {
        self.edit(|c| c.set_name(item, ru))
    }

    fn pick(&self, item: &str, prop: &str, value: &str) -> Result<()> {
        self.edit(|c| c.set_pick(item, prop, value))
    }

    fn term(&self, kind: &str, key: &str, ru: &str) -> Result<()> {
        self.edit(|c| c.set_term(kind, key, ru))
    }

    fn verbatim(&self, kind: &str, key: &str, note: &str) -> Result<()> {
        self.edit(|c| c.set_verbatim(kind, key, note))
    }

    fn unverbatim(&self, kind: &str, key: &str) -> Result<()> {
        self.edit(|c| c.clear_verbatim(kind, key))
    }

    fn accept(&self, rule: &str, entity: &str, note: &str) -> Result<()> {
        self.edit(|c| c.set_accept(rule, entity, note))
    }

    fn unaccept(&self, rule: &str, entity: &str) -> Result<()> {
        self.edit(|c| c.clear_accept(rule, entity))
    }

    fn icon(&self, item: &str, lang: &str) -> Option<Icon> {
        let (blob, from) = self.pictures.get(item)?.get(lang)?;
        let vault = Vault::open(&self.vault_dir).ok()?;
        let bytes = vault.get(&BlobId::from_hex(blob.clone())).ok()?;
        Some(probe(bytes, *from))
    }
}

/// Where every shipped item's picture lives, per language, as the last icon run left it.
fn pictures(vault: &Vault) -> Result<icons::Pictures> {
    let built = build::graph(vault)?;
    let de = vault.latest(spec::DE)?;
    let textures = extract::de_textures(&build::blob(vault, &de, spec::TEXTURES)?)?;
    let cards = icons::cards(&built.graph, &built.taxonomy, &built.wfm, &textures);
    Ok(icons::pictures(vault, &built.graph, &cards, &textures))
}

/// What the picture is, as the desktop app will see it.
fn probe(bytes: Vec<u8>, from: Source) -> Icon {
    let format = image::guess_format(&bytes).ok();
    let mime = match format {
        Some(image::ImageFormat::Png) => "image/png",
        Some(image::ImageFormat::Jpeg) => "image/jpeg",
        Some(image::ImageFormat::WebP) => "image/webp",
        _ => "application/octet-stream",
    };
    let (width, height, alpha) = match image::load_from_memory(&bytes) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            let clear = rgba.pixels().any(|p| p.0[3] < 255);
            (rgba.width(), rgba.height(), clear)
        }
        Err(_) => (0, 0, false),
    };
    Icon {
        bytes,
        mime,
        width,
        height,
        alpha,
        from,
    }
}
