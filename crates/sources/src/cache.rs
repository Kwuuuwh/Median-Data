use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use oozextract::Extractor;

const MAGIC: u32 = 0x1867_C64E;
const ENTRY_LEN: usize = 96;
/// First byte of a block Oodle packed with Kraken; anything else is LZF.
const KRAKEN: u8 = 0x8C;

/// One file the table of contents lists, and where its bytes sit in the `.cache`.
#[derive(Debug, Clone)]
pub struct TocEntry {
    /// Full in-game path, e.g. `/Lotus/Language/Languages.bin`.
    pub path: String,
    pub offset: u64,
    /// Bytes the `.cache` holds, compressed.
    pub stored_len: u32,
    pub len: u32,
    /// When the game last wrote this file.
    pub written_ms: i64,
}

/// A `.toc`/`.cache` pair of the game's `Cache.Windows`.
pub struct Archive {
    entries: Vec<TocEntry>,
    cache: PathBuf,
}

impl Archive {
    /// Open the pair a `.toc` path names and read its table of contents.
    pub fn open(toc: &Path) -> Result<Self> {
        let cache = toc.with_extension("cache");
        if !cache.is_file() {
            bail!("no .cache beside {}", toc.display());
        }
        let raw = std::fs::read(toc).with_context(|| format!("read {}", toc.display()))?;
        let entries = parse_toc(&raw).with_context(|| format!("parse {}", toc.display()))?;
        Ok(Self { entries, cache })
    }

    pub fn entries(&self) -> &[TocEntry] {
        &self.entries
    }

    /// Read one entry, decompressing the blocks the game stored it in.
    pub fn read(&self, entry: &TocEntry) -> Result<Vec<u8>> {
        let mut file = File::open(&self.cache)?;
        file.seek(SeekFrom::Start(entry.offset))?;
        let mut stored = vec![0u8; entry.stored_len as usize];
        file.read_exact(&mut stored)
            .with_context(|| format!("read {}", entry.path))?;
        if entry.stored_len == entry.len {
            return Ok(stored);
        }
        unpack(&stored, entry.len as usize).with_context(|| format!("unpack {}", entry.path))
    }
}

fn parse_toc(raw: &[u8]) -> Result<Vec<TocEntry>> {
    let head = raw.get(..8).context("table of contents is truncated")?;
    let magic = u32::from_le_bytes(head[0..4].try_into().unwrap());
    if magic != MAGIC {
        bail!("not a table of contents: magic {magic:#010x}");
    }
    let version = u32::from_le_bytes(head[4..8].try_into().unwrap());
    if version != 16 && version != 20 {
        bail!("unsupported archive version {version}");
    }
    let body = &raw[8..];
    if body.len() % ENTRY_LEN != 0 {
        bail!(
            "{} trailing bytes after the last entry",
            body.len() % ENTRY_LEN
        );
    }

    // The root is implicit, and a parent is named by its position among directories.
    let mut dir_paths = vec![String::new()];
    let mut files = Vec::new();
    for chunk in body.chunks_exact(ENTRY_LEN) {
        let offset = i64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let stamp = i64::from_le_bytes(chunk[8..16].try_into().unwrap());
        let stored_len = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
        let len = u32::from_le_bytes(chunk[20..24].try_into().unwrap());
        let parent = u32::from_le_bytes(chunk[28..32].try_into().unwrap()) as usize;
        let name = entry_name(&chunk[32..ENTRY_LEN]);
        if offset == -1 {
            let base = dir_paths
                .get(parent)
                .ok_or_else(|| anyhow!("directory {name} sits under unread directory {parent}"))?;
            dir_paths.push(format!("{base}/{name}"));
        } else if stamp != 0 {
            files.push((parent, name, offset as u64, stored_len, len, unix_ms(stamp)));
        }
    }

    files
        .into_iter()
        .map(|(parent, name, offset, stored_len, len, written_ms)| {
            let base = dir_paths
                .get(parent)
                .ok_or_else(|| anyhow!("file {name} sits under unread directory {parent}"))?;
            Ok(TocEntry {
                path: format!("{base}/{name}"),
                offset,
                stored_len,
                len,
                written_ms,
            })
        })
        .collect()
}

/// A Windows file time, in milliseconds since the Unix epoch.
fn unix_ms(stamp: i64) -> i64 {
    stamp / 10_000 - 11_644_473_600_000
}

fn entry_name(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

fn unpack(stored: &[u8], len: usize) -> Result<Vec<u8>> {
    let mut out = vec![0u8; len];
    let mut read = 0;
    let mut written = 0;
    while written < len {
        // No header: the game stored the whole file as one block.
        let (comp, decomp) = match block_lens(&stored[read..]) {
            Some(lens) => {
                read += 8;
                lens
            }
            None => (stored.len() - read, len - written),
        };
        if comp == 0 || read + comp > stored.len() || written + decomp > len {
            bail!("block at {read} runs past the file");
        }
        let block = &stored[read..read + comp];
        let target = &mut out[written..written + decomp];
        if block[0] == KRAKEN {
            Extractor::new()
                .read_from_slice(block, target)
                .map_err(|e| anyhow!("kraken block at {read}: {e}"))?;
        } else if comp == decomp {
            target.copy_from_slice(block);
        } else {
            let plain =
                lzf::decompress(block, decomp).map_err(|e| anyhow!("lzf block at {read}: {e}"))?;
            if plain.len() != decomp {
                bail!("lzf block at {read} gave {} of {decomp} bytes", plain.len());
            }
            target.copy_from_slice(&plain);
        }
        read += comp;
        written += decomp;
    }
    Ok(out)
}

/// Compressed and decompressed length of a block, if the eight bytes are a block header.
fn block_lens(stored: &[u8]) -> Option<(usize, usize)> {
    let head: [u8; 8] = stored.get(..8)?.try_into().ok()?;
    if head[0] != 0x80 || head[7] & 0x0F != 1 {
        return None;
    }
    let comp = (u32::from_be_bytes([head[0], head[1], head[2], head[3]]) >> 2) & 0xFF_FFFF;
    let decomp = (u32::from_be_bytes([head[4], head[5], head[6], head[7]]) >> 5) & 0xFF_FFFF;
    Some((comp as usize, decomp as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toc_entry(offset: i64, stamp: i64, parent: u32, name: &str) -> Vec<u8> {
        let mut raw = Vec::with_capacity(ENTRY_LEN);
        raw.extend_from_slice(&offset.to_le_bytes());
        raw.extend_from_slice(&stamp.to_le_bytes());
        raw.extend_from_slice(&7u32.to_le_bytes());
        raw.extend_from_slice(&9u32.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&parent.to_le_bytes());
        raw.extend_from_slice(name.as_bytes());
        raw.resize(ENTRY_LEN, 0);
        raw
    }

    fn block_head(comp: usize, decomp: usize) -> [u8; 8] {
        let num1 = 0x8000_0000u32 | ((comp as u32) << 2);
        let num2 = ((decomp as u32) << 5) | 1;
        let mut head = [0u8; 8];
        head[0..4].copy_from_slice(&num1.to_be_bytes());
        head[4..8].copy_from_slice(&num2.to_be_bytes());
        head
    }

    #[test]
    fn toc_puts_files_under_their_directories() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&MAGIC.to_le_bytes());
        raw.extend_from_slice(&20u32.to_le_bytes());
        raw.extend(toc_entry(-1, 1, 0, "Lotus"));
        raw.extend(toc_entry(-1, 1, 1, "Language"));
        raw.extend(toc_entry(64, 1, 2, "Languages.bin"));
        raw.extend(toc_entry(128, 0, 2, "Gone.bin"));

        let entries = parse_toc(&raw).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "/Lotus/Language/Languages.bin");
        assert_eq!(entries[0].offset, 64);
        assert_eq!(entries[0].stored_len, 7);
        assert_eq!(entries[0].len, 9);
        assert_eq!(entries[0].written_ms, unix_ms(1));
    }

    #[test]
    fn a_windows_file_time_becomes_a_unix_one() {
        // 2026-08-19T21:09:03Z, as the game wrote it.
        assert_eq!(unix_ms(134_316_473_430_000_000), 1_787_173_743_000);
    }

    #[test]
    fn toc_refuses_a_foreign_file() {
        let raw = vec![0u8; 8];
        assert!(parse_toc(&raw).unwrap_err().to_string().contains("magic"));
    }

    #[test]
    fn block_header_carries_both_lengths() {
        assert_eq!(
            block_lens(&block_head(1234, 0x40000)),
            Some((1234, 0x40000))
        );
    }

    #[test]
    fn a_block_without_a_header_is_the_whole_file() {
        assert_eq!(block_lens(b"\x8C\x00\x00\x00\x00\x00\x00\x00"), None);
        assert_eq!(block_lens(b"\x80\x00"), None);
    }

    #[test]
    fn unpack_reads_an_lzf_block() {
        let plain = b"Cephalon Simaris, Cephalon Cy, Cephalon Suda".repeat(4);
        let packed = lzf::compress(&plain).unwrap();
        let mut stored = block_head(packed.len(), plain.len()).to_vec();
        stored.extend_from_slice(&packed);

        assert_eq!(unpack(&stored, plain.len()).unwrap(), plain);
    }

    #[test]
    fn unpack_copies_a_block_left_plain() {
        let plain = b"Baro Ki'Teer".to_vec();
        let mut stored = block_head(plain.len(), plain.len()).to_vec();
        stored.extend_from_slice(&plain);

        assert_eq!(unpack(&stored, plain.len()).unwrap(), plain);
    }
}
