mod bits;
mod layout;
mod table;

use anyhow::{Context, Result, bail};
use zstd::bulk::Decompressor;

use crate::frame;
use bits::Bits;

/// A type of the game and the properties it sets over its parent's.
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub path: String,
    pub parent: Option<String>,
    /// The type's own properties, in the game's text notation.
    pub text: Option<String>,
}

/// Every type `Packages.bin` holds, in the order its table lists them.
pub fn types(raw: &[u8]) -> Result<Vec<Type>> {
    let layout = layout::locate(raw)?;
    let entries = table::read(layout.table, layout.count)?;
    let mut zstd = Decompressor::with_dictionary(layout.dict)?;
    let mut flags = Bits::new(layout.flags);
    let mut sizes = layout.sizes;
    let mut frames = layout.frames;

    let mut types = Vec::with_capacity(entries.len());
    for entry in entries {
        let text = if flags.next()? {
            let (len, used) = frame::uleb128(sizes).context("stored length")?;
            sizes = &sizes[used..];
            let stored = frames
                .get(..len)
                .with_context(|| format!("{} runs past the stored text", entry.path))?;
            frames = &frames[len..];
            let plain = if flags.next()? {
                frame::unpack(stored, &mut zstd)
                    .with_context(|| format!("unpack {}", entry.path))?
            } else {
                stored.to_vec()
            };
            Some(text(&entry.path, plain)?)
        } else {
            None
        };
        types.push(Type {
            path: entry.path,
            parent: entry.parent,
            text,
        });
    }
    if !frames.is_empty() || !sizes.is_empty() {
        bail!(
            "{} bytes of stored text and {} of lengths belong to no type",
            frames.len(),
            sizes.len()
        );
    }
    Ok(types)
}

fn text(path: &str, mut plain: Vec<u8>) -> Result<String> {
    while plain.last() == Some(&0) {
        plain.pop();
    }
    String::from_utf8(plain).with_context(|| format!("{path} is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINES: [&str; 6] = [
        "ItemType=/Lotus/Types/Items/MiscItems/Ferrite\n",
        "ItemCount=600\n",
        "BuildPrice=15000\n",
        "LocalizeTag=/Lotus/Language/Items/ArcDroneName\n",
        "DefaultWeapon=ZanukaPetMeleeWeaponPS\n",
        "ProductCategory=Sentinels\n",
    ];

    fn dictionary() -> Vec<u8> {
        let samples: Vec<String> = (0..400)
            .map(|i| {
                format!(
                    "{}{}ItemCount={i}\n{}",
                    LINES[i % 6],
                    LINES[(i + 1) % 6],
                    LINES[(i + 3) % 6]
                )
            })
            .collect();
        zstd::dict::from_samples(&samples, 4096).unwrap()
    }

    fn chunk(out: &mut Vec<u8>, body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
    }

    fn text(out: &mut Vec<u8>, text: &str) {
        chunk(out, text.as_bytes());
    }

    /// Name, parent, own text, and whether that text is compressed.
    type Spec<'a> = (&'a str, &'a str, Option<&'a str>, bool);

    /// A package table of directories and the types in each, with `spare` stray bytes
    /// after the last text.
    fn package(dirs: &[(&str, &[Spec])], spare: usize) -> Vec<u8> {
        let dict = dictionary();
        let mut bits: Vec<bool> = Vec::new();
        let mut sizes = (dict.len() as u32).to_le_bytes().to_vec();
        let mut packed = dict.clone();
        let mut table = Vec::new();
        let mut count = 0u32;
        for (dir, types) in dirs {
            text(&mut table, dir);
            table.push(1);
            table.extend_from_slice(&(types.len() as u32).to_le_bytes());
            for (name, parent, own, compressed) in *types {
                text(&mut table, name);
                table.extend_from_slice(&[0x01, 0x00, 0x01]);
                text(&mut table, parent);
                count += 1;
                bits.push(own.is_some());
                let Some(own) = own else { continue };
                bits.push(*compressed);
                let stored = if *compressed {
                    let whole = zstd::bulk::Compressor::with_dictionary(3, &dict)
                        .unwrap()
                        .compress(own.as_bytes())
                        .unwrap();
                    let mut stored = vec![own.len() as u8];
                    stored.extend_from_slice(&whole[frame::MAGIC.len()..]);
                    stored
                } else {
                    format!("{own}\0").into_bytes()
                };
                sizes.push(stored.len() as u8);
                packed.extend_from_slice(&stored);
            }
        }
        packed.extend(std::iter::repeat_n(0x5A, spare));
        let mut flags = vec![0u8; bits.len().div_ceil(8)];
        for (i, bit) in bits.iter().enumerate() {
            flags[i / 8] |= u8::from(*bit) << (i % 8);
        }

        let mut raw = vec![0u8; 16];
        for number in [20u32, 46, 1] {
            raw.extend_from_slice(&number.to_le_bytes());
        }
        raw.extend_from_slice(&[0xAB; 7]);
        chunk(&mut raw, &flags);
        raw.extend_from_slice(&[0xAB; 5]);
        chunk(&mut raw, &sizes);
        raw.extend_from_slice(&[0xAB; 3]);
        chunk(&mut raw, &packed);
        raw.extend_from_slice(&count.to_le_bytes());
        raw.extend_from_slice(&[0xEE; 13]);
        raw.extend_from_slice(&table);
        raw
    }

    fn hounds(spare: usize) -> Vec<u8> {
        package(
            &[
                (
                    "/Lotus/Types/Friendly/Pets/ZanukaPets/",
                    &[
                        (
                            "ZanukaPetCPowerSuit",
                            "ZanukaPetPowerSuit",
                            Some("DefaultWeapon=ZanukaPetMeleeWeaponPS\n"),
                            true,
                        ),
                        (
                            "ZanukaPetPowerSuit",
                            "/EE/Types/Game/PowerSuit",
                            None,
                            false,
                        ),
                    ],
                ),
                (
                    "/EE/Editor/",
                    &[(
                        "BrowserRect",
                        "/EE/Types/WinRes/WinRect",
                        Some("SizeY=600\n"),
                        false,
                    )],
                ),
            ],
            spare,
        )
    }

    #[test]
    fn every_type_comes_with_its_own_text() {
        let types = types(&hounds(0)).unwrap();

        assert_eq!(types.len(), 3);
        assert_eq!(
            types[0],
            Type {
                path: "/Lotus/Types/Friendly/Pets/ZanukaPets/ZanukaPetCPowerSuit".into(),
                parent: Some("/Lotus/Types/Friendly/Pets/ZanukaPets/ZanukaPetPowerSuit".into()),
                text: Some("DefaultWeapon=ZanukaPetMeleeWeaponPS\n".into()),
            }
        );
        assert_eq!(types[1].text, None);
        assert_eq!(types[2].text.as_deref(), Some("SizeY=600\n"));
    }

    #[test]
    fn stored_text_no_type_accounts_for_is_refused() {
        assert!(types(&hounds(1)).is_err());
    }

    #[test]
    fn a_foreign_version_is_refused() {
        let mut raw = hounds(0);
        raw[20] = 45;

        let error = types(&raw).unwrap_err().to_string();

        assert!(error.contains("version"), "{error}");
    }
}
