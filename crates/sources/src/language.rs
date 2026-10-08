use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use zstd::bulk::Decompressor;

use crate::frame;

const HASH_LEN: usize = 16;
const VERSION: u32 = 20;
/// Two header numbers the format does not explain.
const UNREAD_HEADER: usize = 8;
/// A phrase flagged this way is a ULEB128 length and a magicless zstd frame.
const PACKED: u32 = 0x0200_0000;

/// Every phrase `Languages.bin` holds, keyed by section path plus record key.
pub fn strings(raw: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut reader = Reader::new(raw);
    reader.skip(HASH_LEN)?;
    let version = reader.u32()?;
    if version != VERSION {
        bail!("unsupported language table version {version}");
    }
    reader.skip(UNREAD_HEADER)?;
    let languages = reader.u32()?;
    for _ in 0..languages {
        reader.chunk().context("language suffix")?;
    }
    let mut zstd = Decompressor::with_dictionary(reader.chunk().context("shared dictionary")?)?;

    let mut phrases = BTreeMap::new();
    let sections = reader.u32()?;
    for _ in 0..sections {
        let path = reader.text().context("section path")?;
        let pool = reader.chunk().context("section pool")?;
        let records = reader.u32()?;
        for _ in 0..records {
            let key = reader.text().context("record key")?;
            let offset = reader.u32()? as usize;
            let stamp = reader.u32()?;
            let len = (stamp & 0xFFFF) as usize;
            let stored = pool
                .get(offset..offset + len)
                .with_context(|| format!("{path}{key} points past its pool"))?;
            let phrase = if stamp & PACKED == 0 {
                std::str::from_utf8(stored)
                    .with_context(|| format!("{path}{key} is not UTF-8"))?
                    .to_string()
            } else {
                unpack(stored, &mut zstd).with_context(|| format!("unpack {path}{key}"))?
            };
            phrases.insert(format!("{path}{key}"), phrase);
        }
    }
    Ok(phrases)
}

fn unpack(stored: &[u8], zstd: &mut Decompressor) -> Result<String> {
    Ok(String::from_utf8(frame::unpack(stored, zstd)?)?)
}

struct Reader<'a> {
    raw: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self { raw, pos: 0 }
    }

    fn skip(&mut self, len: usize) -> Result<()> {
        self.bytes(len).map(|_| ())
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(len).context("length overflows")?;
        let run = self
            .raw
            .get(self.pos..end)
            .with_context(|| format!("wanted {len} bytes at {}", self.pos))?;
        self.pos = end;
        Ok(run)
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    /// A run of bytes behind its own length.
    fn chunk(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.bytes(len)
    }

    fn text(&mut self) -> Result<&'a str> {
        let run = self.chunk()?;
        std::str::from_utf8(run).context("not UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zstd::bulk::Compressor;

    const DICTIONARY: &[u8] = "Шаг вперед, шаг назад, Шаг в сторону".as_bytes();

    fn chunk(raw: &[u8]) -> Vec<u8> {
        let mut out = (raw.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(raw);
        out
    }

    fn packed(phrase: &str) -> Vec<u8> {
        let whole = Compressor::with_dictionary(3, DICTIONARY)
            .unwrap()
            .compress(phrase.as_bytes())
            .unwrap();
        let mut stored = vec![phrase.len() as u8];
        stored.extend_from_slice(&whole[frame::MAGIC.len()..]);
        stored
    }

    fn table(records: &[(&str, Vec<u8>, bool)]) -> Vec<u8> {
        let mut pool = Vec::new();
        let mut entries = Vec::new();
        for (key, stored, is_packed) in records {
            let offset = pool.len() as u32;
            pool.extend_from_slice(stored);
            let len = stored.len() as u32;
            entries.push((*key, offset, if *is_packed { len | PACKED } else { len }));
        }

        let mut raw = vec![0u8; HASH_LEN];
        raw.extend_from_slice(&VERSION.to_le_bytes());
        raw.extend_from_slice(&[0u8; UNREAD_HEADER]);
        raw.extend_from_slice(&1u32.to_le_bytes());
        raw.extend(chunk(b"_ru"));
        raw.extend(chunk(DICTIONARY));
        raw.extend_from_slice(&1u32.to_le_bytes());
        raw.extend(chunk(b"/EE_Menus/"));
        raw.extend(chunk(&pool));
        raw.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (key, offset, stamp) in entries {
            raw.extend(chunk(key.as_bytes()));
            raw.extend_from_slice(&offset.to_le_bytes());
            raw.extend_from_slice(&stamp.to_le_bytes());
        }
        raw
    }

    #[test]
    fn a_packed_phrase_comes_back_whole() {
        let raw = table(&[("Action_WALK_FORWARD", packed("Шаг вперед"), true)]);
        let phrases = strings(&raw).unwrap();
        assert_eq!(phrases["/EE_Menus/Action_WALK_FORWARD"], "Шаг вперед");
    }

    #[test]
    fn a_plain_phrase_is_read_as_it_stands() {
        let raw = table(&[("Action_WALK_BACK", "Шаг назад".into(), false)]);
        let phrases = strings(&raw).unwrap();
        assert_eq!(phrases["/EE_Menus/Action_WALK_BACK"], "Шаг назад");
    }

    #[test]
    fn a_foreign_version_is_refused() {
        let mut raw = table(&[("Action_WALK_BACK", "Шаг назад".into(), false)]);
        raw[HASH_LEN] = 19;
        assert!(strings(&raw).unwrap_err().to_string().contains("version"));
    }
}
