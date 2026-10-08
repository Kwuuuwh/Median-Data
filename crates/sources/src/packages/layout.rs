use anyhow::{Context, Result, bail};

use crate::frame;

const HASH_LEN: usize = 16;
const HEADER_LEN: u32 = 20;
const VERSION: u32 = 46;
/// Hash, header length, version and one more number ahead of everything else.
const PREAMBLE_LEN: usize = HASH_LEN + 12;
const DICT_MAGIC: [u8; 4] = [0x37, 0xA4, 0x30, 0xEC];
/// Most filler bytes between the type count and the first directory.
const TABLE_GAP: usize = 64;
/// Longest a directory path runs.
const DIR_LEN: usize = 1024;

/// Where each part of `Packages.bin` sits.
pub struct Layout<'a> {
    /// A has-text bit per type, and an is-compressed bit after each that has text.
    pub flags: &'a [u8],
    /// A ULEB128 stored length per type with text.
    pub sizes: &'a [u8],
    pub dict: &'a [u8],
    /// Each type's stored text, back to back in table order.
    pub frames: &'a [u8],
    pub table: &'a [u8],
    pub count: usize,
}

/// Find each part by what it holds, past the filler of varying length between them.
pub fn locate(raw: &[u8]) -> Result<Layout<'_>> {
    if u32_at(raw, HASH_LEN) != Some(HEADER_LEN) {
        bail!("not a package table");
    }
    let version = u32_at(raw, HASH_LEN + 4).context("package table is truncated")?;
    if version != VERSION {
        bail!("unsupported package table version {version}");
    }

    let dict_at = raw
        .windows(DICT_MAGIC.len())
        .position(|run| run == DICT_MAGIC)
        .context("no compression dictionary")?;
    let packed_at = dict_at.checked_sub(4).context("dictionary has no length")?;
    let packed_len = u32_at(raw, packed_at).unwrap() as usize;
    let packed = raw
        .get(dict_at..dict_at + packed_len)
        .context("compressed text runs past the file")?;
    let count_at = dict_at + packed_len;
    let count = u32_at(raw, count_at).context("no type count")? as usize;
    let table_at = first_dir(raw, count_at + 4).context("no type table")?;

    let (sizes_at, dict_len, texts) =
        find_sizes(raw, packed_at, packed.len()).context("no stored lengths")?;
    let flags_len = (count + texts).div_ceil(8);
    let flags_at = (PREAMBLE_LEN..sizes_at)
        .find(|&at| u32_at(raw, at) == Some(flags_len as u32) && at + 4 + flags_len <= sizes_at)
        .context("no text flags")?;

    let sizes_len = u32_at(raw, sizes_at).unwrap() as usize;
    Ok(Layout {
        flags: &raw[flags_at + 4..flags_at + 4 + flags_len],
        sizes: &raw[sizes_at + 8..sizes_at + 4 + sizes_len],
        dict: &packed[..dict_len],
        frames: &packed[dict_len..],
        table: &raw[table_at..],
        count,
    })
}

/// The stored lengths: a dictionary length, then one ULEB128 per text, together spanning
/// the compressed part exactly. Gives where they start, the dictionary length and how
/// many texts there are.
fn find_sizes(raw: &[u8], end: usize, packed_len: usize) -> Option<(usize, usize, usize)> {
    (PREAMBLE_LEN..end).rev().find_map(|at| {
        let len = u32_at(raw, at)? as usize;
        if len < 4 || at + 4 + len > end {
            return None;
        }
        let dict_len = u32_at(raw, at + 4)? as usize;
        if dict_len == 0 || dict_len >= packed_len {
            return None;
        }
        let texts = spans(&raw[at + 8..at + 4 + len], packed_len - dict_len)?;
        Some((at, dict_len, texts))
    })
}

/// How many ULEB128 lengths the run holds, if they add up to exactly `total`.
fn spans(mut run: &[u8], total: usize) -> Option<usize> {
    let mut sum = 0usize;
    let mut count = 0;
    while !run.is_empty() {
        let (len, used) = frame::uleb128(run).ok()?;
        sum = sum.checked_add(len)?;
        if sum > total {
            return None;
        }
        run = &run[used..];
        count += 1;
    }
    (sum == total).then_some(count)
}

/// The first directory record at or shortly after `from`: a path wrapped in slashes,
/// then a marker byte of 1.
fn first_dir(raw: &[u8], from: usize) -> Option<usize> {
    (from..=from + TABLE_GAP).find(|&at| {
        let Some(len) = u32_at(raw, at).map(|len| len as usize) else {
            return false;
        };
        (2..=DIR_LEN).contains(&len)
            && raw.get(at + 4) == Some(&b'/')
            && raw.get(at + 3 + len) == Some(&b'/')
            && raw.get(at + 4 + len) == Some(&1)
    })
}

fn u32_at(raw: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(raw.get(at..at + 4)?.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_count_only_when_they_fill_the_span() {
        assert_eq!(spans(&[3, 0x81, 0x01], 132), Some(2));
        assert_eq!(spans(&[3, 4], 8), None);
        assert_eq!(spans(&[], 0), Some(0));
    }

    #[test]
    fn the_table_starts_past_its_filler() {
        let mut raw = vec![0xEE; 13];
        raw.extend_from_slice(&10u32.to_le_bytes());
        raw.extend_from_slice(b"/EE/Types/");
        raw.push(1);

        assert_eq!(first_dir(&raw, 0), Some(13));
    }
}
