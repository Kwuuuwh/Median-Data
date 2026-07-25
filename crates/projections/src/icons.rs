use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result};
use image::ExtendedColorType;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use rusqlite::Transaction;

use crate::projection::{Context, Projection, Summary};

/// Raw icon bytes for an item in one language, however the build pinned them.
pub trait Source {
    fn bytes(&self, unique_name: &str, lang: &str) -> Option<Vec<u8>>;
}

/// Icons for what the product ships, re-encoded to one size and format. Items sharing
/// artwork share a file: the name is the content hash, so the pack dedupes itself. A mod
/// carries its name and stats in the picture, so it gets one per language; everything else
/// resolves to the same file in both.
pub struct Icons;

/// Pictures are fitted inside this edge; sources arrive at assorted sizes, and a mod card
/// is taller than it is wide.
const EDGE: u32 = 128;
const DIR: &str = "pack";

const SETUP: &str = "\
CREATE TABLE item_icons (
  unique_name TEXT NOT NULL,
  lang        TEXT NOT NULL,
  image_id    TEXT NOT NULL,
  PRIMARY KEY (unique_name, lang)
) WITHOUT ROWID;
CREATE INDEX idx_item_icons_image ON item_icons(image_id);
CREATE TABLE vendor_icons (
  vendor   TEXT PRIMARY KEY,
  image_id TEXT NOT NULL
) WITHOUT ROWID;";

impl Projection for Icons {
    fn name(&self) -> &'static str {
        "icons"
    }

    fn db(&self, tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<Option<Summary>> {
        tx.execute_batch(SETUP)?;
        let Some(source) = ctx.icons else {
            return Ok(Some(Summary {
                name: self.name(),
                detail: "no icons pinned — run `icons` first".to_string(),
            }));
        };

        let dir = ctx.out.join(DIR);
        fs::create_dir_all(&dir)?;
        let mut insert =
            tx.prepare("INSERT INTO item_icons (unique_name, lang, image_id) VALUES (?1, ?2, ?3)")?;

        // One encode per distinct source image, however many items and languages point at it.
        let mut encoded: BTreeMap<String, String> = BTreeMap::new();
        let (mut linked, mut written, mut failed, mut split) = (0, 0, 0, 0);

        for item in ctx.graph.items() {
            if !ctx.scope.allows(&item.unique_name) {
                continue;
            }
            let mut ids = BTreeMap::new();
            for lang in graph::LANGS {
                let Some(bytes) = source.bytes(&item.unique_name, lang) else {
                    continue;
                };
                // The pack file is named after the SOURCE hash, so an already-encoded image
                // is recognised on disk and skipped — a rebuild re-encodes nothing.
                let key = blake3::hash(&bytes).to_hex().to_string();
                let id = match encoded.get(&key) {
                    Some(id) => id.clone(),
                    None => {
                        let id = key[..16].to_string();
                        let path = dir.join(format!("{id}.webp"));
                        if !path.exists() {
                            match convert(&bytes) {
                                Ok(webp) => {
                                    fs::write(&path, &webp)?;
                                    written += 1;
                                }
                                Err(e) => {
                                    eprintln!("skip icon for {} ({lang}): {e:#}", item.unique_name);
                                    failed += 1;
                                    continue;
                                }
                            }
                        }
                        encoded.insert(key.clone(), id.clone());
                        id
                    }
                };
                insert.execute((&item.unique_name, lang, &id))?;
                ids.insert(*lang, id);
            }
            if ids.is_empty() {
                continue;
            }
            linked += 1;
            if ids.values().collect::<std::collections::BTreeSet<_>>().len() > 1 {
                split += 1;
            }
        }

        // Vendors are people and places rather than items, and the wiki illustrates them; one
        // picture each, the same in both languages.
        let mut vendor_icon =
            tx.prepare("INSERT INTO vendor_icons (vendor, image_id) VALUES (?1, ?2)")?;
        let mut vendors = 0;
        for node in ctx.graph.nodes() {
            let graph::Node::Vendor(v) = node else { continue };
            let Some(bytes) = source.bytes(&node.id(), "ru") else {
                continue;
            };
            let key = blake3::hash(&bytes).to_hex().to_string();
            let id = match encoded.get(&key) {
                Some(id) => id.clone(),
                None => {
                    let id = key[..16].to_string();
                    let path = dir.join(format!("{id}.webp"));
                    if !path.exists() {
                        match convert(&bytes) {
                            Ok(webp) => {
                                fs::write(&path, &webp)?;
                                written += 1;
                            }
                            Err(e) => {
                                eprintln!("skip picture for {}: {e:#}", v.key);
                                failed += 1;
                                continue;
                            }
                        }
                    }
                    encoded.insert(key.clone(), id.clone());
                    id
                }
            };
            vendor_icon.execute((&v.key, &id))?;
            vendors += 1;
        }

        let swept = sweep(&dir, &encoded)?;

        Ok(Some(Summary {
            name: self.name(),
            detail: format!(
                "{linked} items and {vendors} vendors over {} images ({split} differ by language, \
                 {written} written, {swept} stale removed, {failed} unreadable)",
                encoded.len()
            ),
        }))
    }
}

/// Drop pack files nothing points at any more, so a narrowing scope leaves no litter.
fn sweep(dir: &Path, keep: &BTreeMap<String, String>) -> Result<usize> {
    let mut removed = 0;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if !keep.values().any(|id| *id == stem) {
            fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Decode whatever DE served and re-encode it as lossless WebP at a fixed size.
fn convert(bytes: &[u8]) -> Result<Vec<u8>> {
    let image = image::load_from_memory(bytes).context("decode")?;
    let fitted = image.resize(EDGE, EDGE, FilterType::Lanczos3).to_rgba8();
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .encode(
            fitted.as_raw(),
            fitted.width(),
            fitted.height(),
            ExtendedColorType::Rgba8,
        )
        .context("encode webp")?;
    Ok(out)
}

