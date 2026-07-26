use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use funnel::State;
use serde::Serialize;
use vault::BlobId;

/// What the build stamped the catalog with, repeated in the manifest so a client can read it
/// before downloading the database itself.
pub struct Stamp {
    pub schema: u32,
    pub fetched_ms: i64,
    /// The pinned snapshot each source was at, so the next run can tell whether anything moved
    /// without holding a vault of its own.
    pub sources: BTreeMap<String, String>,
}

/// Where the publishable artifacts are written.
const DIST: &str = "dist";
/// Names an application asks for by. They never change, so a client can be written once.
const DB: &str = "catalog.sqlite.gz";
const PACK: &str = "pack.tar";
const INDEX: &str = "pack.index";
const MANIFEST: &str = "manifest.json";

/// What a client needs to know before it downloads anything: whether it can read this schema
/// at all, what the version is, and how many bytes each part costs.
#[derive(Serialize)]
struct Manifest {
    schema: u32,
    version: String,
    /// When the game export this was built from was published.
    fetched_ms: i64,
    sources: BTreeMap<String, String>,
    db: Part,
    pack: Pack,
    /// Everything a fresh install downloads, so a progress bar can start at zero.
    total: u64,
}

#[derive(Serialize)]
struct Part {
    file: String,
    blake3: String,
    size: u64,
    /// Size after decompression, where the file is compressed.
    #[serde(skip_serializing_if = "Option::is_none")]
    unpacked: Option<u64>,
}

#[derive(Serialize)]
struct Pack {
    #[serde(flatten)]
    part: Part,
    /// How many pictures the pack holds.
    count: usize,
}

/// One picture in the pack: where its bytes sit inside the archive and what they hash to.
/// A client diffs this against what it holds and asks the release for exactly the byte
/// ranges it lacks, so how far behind it is costs nothing but the pictures that changed.
struct Entry {
    id: String,
    offset: u64,
    len: u64,
    blake3: String,
}

/// Package what the last build produced. Nothing is fetched, nothing is rebuilt and nothing is
/// uploaded: this only turns the artifacts on disk into the files a release is made of, so the
/// same command produces the same release from the same build.
pub fn run(
    db: &Path,
    pack: &Path,
    state: &Path,
    stamp: Stamp,
    previous: Option<&Path>,
) -> Result<()> {
    let state: State = serde_json::from_slice(&fs::read(state).with_context(|| {
        format!(
            "read {} — run `build` first, the version comes from it",
            state.display()
        )
    })?)?;
    let version = state
        .version
        .clone()
        .context("the build wrote no version into its state")?;
    let Stamp {
        schema,
        fetched_ms,
        sources,
    } = stamp;

    if let Some(dir) = previous {
        let was = released(dir)?;
        if was.as_deref() == Some(version.as_str()) {
            bail!("version {version} is already released — nothing changed since it was built");
        }
        if let Some(was) = was {
            let (major, _) = split(&was);
            if major != schema {
                eprintln!(
                    "release  schema {major} -> {schema}: an application built for {was} cannot \
                     read this one"
                );
            }
        }
    }

    let dist = PathBuf::from(DIST);
    fs::create_dir_all(&dist)?;
    let db_part = gzip(db, &dist.join(DB))?;
    let ids = pack_ids(pack)?;
    let (pack_part, entries) = tar(pack, &dist.join(PACK), &ids)?;

    let manifest = Manifest {
        schema,
        version: version.clone(),
        fetched_ms,
        sources,
        total: db_part.size + pack_part.size,
        db: db_part,
        pack: Pack {
            part: pack_part,
            count: ids.len(),
        },
    };
    fs::write(dist.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;
    fs::write(dist.join(INDEX), index(&entries))?;
    fs::copy("catalog.state.json", dist.join("catalog.state.json"))?;

    eprintln!(
        "release  {version} — {} ({} MB) + {} pictures ({} MB)",
        DB,
        mb(manifest.db.size),
        manifest.pack.count,
        mb(manifest.pack.part.size)
    );
    eprintln!("release  files are in {DIST}/, upload them yourself");
    Ok(())
}

/// The version of the release a directory holds, from the state file published with it.
fn released(dir: &Path) -> Result<Option<String>> {
    let path = dir.join("catalog.state.json");
    if !path.exists() {
        return Ok(None);
    }
    let state: State = serde_json::from_slice(&fs::read(path)?)?;
    Ok(state.version)
}

/// Every picture in the pack, by file name. Names are content hashes, so the set alone says
/// what a client already holds.
fn pack_ids(pack: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for entry in fs::read_dir(pack).with_context(|| format!("read {}", pack.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "webp")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            out.insert(stem.to_string());
        }
    }
    Ok(out)
}

/// One line per picture: name, where it starts in the archive, how long it is, and the
/// short hash of those bytes.
fn index(entries: &[Entry]) -> String {
    let mut out = String::with_capacity(entries.len() * 48);
    for e in entries {
        out.push_str(&format!("{} {} {} {}\n", e.id, e.offset, e.len, e.blake3));
    }
    out
}

/// Compress one file, reporting what came out.
fn gzip(from: &Path, to: &Path) -> Result<Part> {
    let raw = fs::read(from).with_context(|| format!("read {}", from.display()))?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw)?;
    let packed = encoder.finish()?;
    fs::write(to, &packed)?;
    Ok(part(to, &packed, Some(raw.len() as u64)))
}

/// Archive the named pictures, in name order so the same set always produces the same file,
/// and read back where each one landed. The positions come from the archive itself rather
/// than from arithmetic over the format, so the index cannot drift from the file it
/// describes.
fn tar(pack: &Path, to: &Path, ids: &BTreeSet<String>) -> Result<(Part, Vec<Entry>)> {
    let file = BufWriter::new(File::create(to)?);
    let mut archive = tar::Builder::new(file);
    archive.mode(tar::HeaderMode::Deterministic);
    for id in ids {
        let name = format!("{id}.webp");
        archive.append_path_with_name(pack.join(&name), &name)?;
    }
    archive.into_inner()?.flush()?;

    let bytes = fs::read(to)?;
    let mut entries = Vec::with_capacity(ids.len());
    for entry in tar::Archive::new(bytes.as_slice()).entries()? {
        let entry = entry?;
        let path = entry.path()?.into_owned();
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let offset = entry.raw_file_position();
        let len = entry.size();
        let at = offset as usize;
        entries.push(Entry {
            id: id.to_string(),
            offset,
            len,
            blake3: short(&bytes[at..at + len as usize]),
        });
    }
    Ok((part(to, &bytes, None), entries))
}

/// How much of a picture's hash the index carries. Enough to catch a corrupt or stale file
/// without spending a megabyte of index on it.
fn short(bytes: &[u8]) -> String {
    BlobId::of(bytes).to_string()[..16].to_string()
}

fn part(path: &Path, bytes: &[u8], unpacked: Option<u64>) -> Part {
    Part {
        file: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        blake3: BlobId::of(bytes).to_string(),
        size: bytes.len() as u64,
        unpacked,
    }
}

fn split(version: &str) -> (u32, String) {
    match version.split_once('.') {
        Some((major, rest)) => (major.parse().unwrap_or(0), rest.to_string()),
        None => (0, version.to_string()),
    }
}

fn mb(bytes: u64) -> u64 {
    bytes / 1_000_000
}
