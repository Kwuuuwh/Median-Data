use anyhow::{Context, Result, bail};

/// Flag bytes each type carries between its name and its parent.
const FLAGS_LEN: usize = 3;

/// One type of the game, as the table lists it.
pub struct Entry {
    pub path: String,
    pub parent: Option<String>,
}

/// Every type the table lists, directory by directory, in the order it lists them.
pub fn read(raw: &[u8], count: usize) -> Result<Vec<Entry>> {
    let mut reader = Reader { raw, at: 0 };
    let mut entries = Vec::with_capacity(count);
    while entries.len() < count {
        let dir = reader.text().context("directory")?;
        reader.skip(1)?;
        let types = reader.u32()?;
        for _ in 0..types {
            let name = reader.text().context("type name")?;
            reader.skip(FLAGS_LEN)?;
            let base = reader.text().context("parent")?;
            let parent = match base {
                "" => None,
                base if base.starts_with('/') => Some(base.to_string()),
                base => Some(format!("{dir}{base}")),
            };
            entries.push(Entry {
                path: format!("{dir}{name}"),
                parent,
            });
        }
    }
    if entries.len() != count || reader.at != raw.len() {
        bail!(
            "the table lists {} types in {} of {} bytes, but promises {count}",
            entries.len(),
            reader.at,
            raw.len()
        );
    }
    Ok(entries)
}

struct Reader<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let run = self
            .raw
            .get(self.at..self.at + len)
            .with_context(|| format!("wanted {len} bytes at {}", self.at))?;
        self.at += len;
        Ok(run)
    }

    fn skip(&mut self, len: usize) -> Result<()> {
        self.bytes(len).map(|_| ())
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn text(&mut self) -> Result<&'a str> {
        let len = self.u32()? as usize;
        std::str::from_utf8(self.bytes(len)?).context("not UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(out: &mut Vec<u8>, text: &str) {
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }

    fn dir(out: &mut Vec<u8>, path: &str, types: &[(&str, &str)]) {
        text(out, path);
        out.push(1);
        out.extend_from_slice(&(types.len() as u32).to_le_bytes());
        for (name, parent) in types {
            text(out, name);
            out.extend_from_slice(&[0x01, 0x00, 0x01]);
            text(out, parent);
        }
    }

    #[test]
    fn a_parent_is_named_whole_or_beside_its_child() {
        let mut raw = Vec::new();
        dir(
            &mut raw,
            "/Lotus/Fx/",
            &[
                ("FastFogG", "/EE/Materials/Defaults/FogPlane"),
                ("FastFogJ", "FastFogG"),
            ],
        );
        dir(&mut raw, "/EE/Types/", &[("Templatizer", "")]);

        let entries = read(&raw, 3).unwrap();

        assert_eq!(entries[0].path, "/Lotus/Fx/FastFogG");
        assert_eq!(
            entries[0].parent.as_deref(),
            Some("/EE/Materials/Defaults/FogPlane")
        );
        assert_eq!(entries[1].parent.as_deref(), Some("/Lotus/Fx/FastFogG"));
        assert_eq!(entries[2].path, "/EE/Types/Templatizer");
        assert_eq!(entries[2].parent, None);
    }

    #[test]
    fn a_table_with_bytes_left_over_is_refused() {
        let mut raw = Vec::new();
        dir(&mut raw, "/EE/Types/", &[("Templatizer", "")]);
        raw.push(0);

        assert!(read(&raw, 1).is_err());
    }
}
