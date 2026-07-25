use serde::{Deserialize, Serialize};

/// One pinned input in a snapshot: a logical name and the blob holding its bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub logical: String,
    pub blob: String,
    pub len: u64,
}

/// A fetch run: the raw inputs pinned together under one source.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Snapshot {
    pub id: String,
    pub source: String,
    pub created_ms: i64,
    pub entries: Vec<Entry>,
}

impl Snapshot {
    pub fn new(id: impl Into<String>, source: impl Into<String>, created_ms: i64) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            created_ms,
            entries: Vec::new(),
        }
    }

    /// The entry for a logical name, if pinned.
    pub fn entry(&self, logical: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.logical == logical)
    }
}
