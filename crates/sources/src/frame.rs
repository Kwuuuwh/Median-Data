use anyhow::{Result, bail};
use zstd::bulk::Decompressor;

pub const MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Inflate a ULEB128 length followed by a magicless zstd frame.
pub fn unpack(stored: &[u8], zstd: &mut Decompressor) -> Result<Vec<u8>> {
    let (len, header) = uleb128(stored)?;
    let mut frame = Vec::with_capacity(MAGIC.len() + stored.len() - header);
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&stored[header..]);
    Ok(zstd.decompress(&frame, len)?)
}

/// The value and how many bytes it spans.
pub fn uleb128(raw: &[u8]) -> Result<(usize, usize)> {
    let mut value = 0usize;
    let mut shift = 0u32;
    for (i, &byte) in raw.iter().enumerate() {
        value |= ((byte & 0x7F) as usize) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
        shift += 7;
        if shift >= usize::BITS {
            break;
        }
    }
    bail!("a length runs past its bytes");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uleb128_reads_a_length_over_one_byte() {
        assert_eq!(uleb128(&[0x13]).unwrap(), (19, 1));
        assert_eq!(uleb128(&[0xE5, 0x8E, 0x26]).unwrap(), (624485, 3));
    }

    #[test]
    fn a_frame_comes_back_without_its_magic() {
        let plain = b"ItemCount=20\n";
        let packed = zstd::bulk::compress(plain, 3).unwrap();
        let mut stored = vec![plain.len() as u8];
        stored.extend_from_slice(&packed[MAGIC.len()..]);

        let mut zstd = Decompressor::new().unwrap();

        assert_eq!(unpack(&stored, &mut zstd).unwrap(), plain);
    }
}
