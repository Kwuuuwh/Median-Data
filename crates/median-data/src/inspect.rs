use std::path::PathBuf;

use anyhow::Result;
use consensus::Source;
use studio::{Icon, Snapshot, Store};
use vault::{BlobId, Vault};

use crate::curation::Curation;
use crate::{build, curation, extract, icons, spec, unmatched};

/// Serve Studio over the pinned sources. Every screen reads one in-memory graph; a curated
/// decision rewrites the file and reassembles it.
pub fn run(vault_dir: &str, addr: &str) -> Result<()> {
    let inspector = Inspector::open(vault_dir)?;
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
}

impl Inspector {
    fn open(vault_dir: &str) -> Result<Self> {
        let vault = Vault::open(vault_dir)?;
        Ok(Self {
            vault_dir: vault_dir.into(),
            scope: PathBuf::from(crate::SCOPE),
            curation: PathBuf::from(crate::CURATION),
            pictures: pictures(&vault).unwrap_or_default(),
        })
    }

    /// Read the decisions, change them, write them back.
    fn edit(&self, change: impl FnOnce(&mut Curation)) -> Result<()> {
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
        let (report, _) = build::judge(&vault, &built, &curated)?;

        let policy = projections::load(&self.scope)?;
        let scope = projections::apply(&built.graph, &policy);

        let mut unresolved = built.orphans;
        unresolved.extend(unmatched::find(&built.wfm, &built.matched));

        let iconless = built
            .graph
            .items()
            .filter(|i| scope.allows(&i.unique_name) && !self.pictures.contains_key(&i.unique_name))
            .map(|i| i.unique_name.clone())
            .collect();

        Ok(Snapshot {
            graph: built.graph,
            taxonomy: built.taxonomy,
            report,
            scope,
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

    fn name(&self, item: &str, ru: &str) -> Result<()> {
        self.edit(|c| c.set_name(item, ru))
    }

    fn pick(&self, item: &str, prop: &str, value: &str) -> Result<()> {
        self.edit(|c| c.set_pick(item, prop, value))
    }

    fn term(&self, kind: &str, key: &str, ru: &str) -> Result<()> {
        self.edit(|c| c.set_term(kind, key, ru))
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
    let cards = icons::cards(&built.graph, &built.wfm, &textures);
    Ok(icons::pictures(vault, &cards, &textures))
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
