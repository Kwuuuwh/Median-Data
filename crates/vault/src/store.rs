use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::blob::BlobId;
use crate::snapshot::Snapshot;

/// Content-addressed store of raw input blobs plus per-source fetch snapshot.
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    /// Open a vault at `root`, creating its layout if absent.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("snapshots"))?;
        Ok(Self { root })
    }

    /// Store bytes under their content id; a no-op when already present.
    pub fn put(&self, bytes: &[u8]) -> Result<BlobId> {
        let id = BlobId::of(bytes);
        let path = self.blob_path(&id);
        if !path.exists() {
            fs::write(&path, bytes).with_context(|| format!("write blob {id}"))?;
        }
        Ok(id)
    }

    /// Read a blob's bytes by content id.
    pub fn get(&self, id: &BlobId) -> Result<Vec<u8>> {
        fs::read(self.blob_path(id)).with_context(|| format!("read blob {id}"))
    }

    /// Persist a snapshot and mark it as its source's latest.
    pub fn save(&self, snap: &Snapshot) -> Result<()> {
        let json = serde_json::to_vec_pretty(snap)?;
        fs::write(self.snapshot_path(&snap.id), json)?;
        fs::write(self.head_path(&snap.source), &snap.id)?;
        Ok(())
    }

    /// Load a snapshot by id.
    pub fn load(&self, id: &str) -> Result<Snapshot> {
        let bytes =
            fs::read(self.snapshot_path(id)).with_context(|| format!("load snapshot {id}"))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Load the latest snapshot recorded for a source.
    pub fn latest(&self, source: &str) -> Result<Snapshot> {
        let id = fs::read_to_string(self.head_path(source))
            .with_context(|| format!("no snapshot for source '{source}' - run fetch first"))?;
        self.load(id.trim())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn blob_path(&self, id: &BlobId) -> PathBuf {
        self.root.join("blobs").join(id.as_str())
    }

    fn snapshot_path(&self, id: &str) -> PathBuf {
        self.root.join("snapshots").join(format!("{id}.json"))
    }

    fn head_path(&self, source: &str) -> PathBuf {
        self.root.join("snapshots").join(format!("{source}.head"))
    }
}
