use anyhow::{Context, Result};

/// Flags read one bit at a time, lowest bit of each byte first.
pub struct Bits<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Bits<'a> {
    pub fn new(raw: &'a [u8]) -> Self {
        Self { raw, at: 0 }
    }

    pub fn next(&mut self) -> Result<bool> {
        let byte = self
            .raw
            .get(self.at / 8)
            .context("the flags run out before the types do")?;
        let bit = byte >> (self.at % 8) & 1 == 1;
        self.at += 1;
        Ok(bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_come_lowest_first() {
        let mut bits = Bits::new(&[0b0000_0101, 0b1]);
        let read: Vec<bool> = (0..9).map(|_| bits.next().unwrap()).collect();
        assert_eq!(
            read,
            [true, false, true, false, false, false, false, false, true]
        );
    }

    #[test]
    fn reading_past_the_last_byte_fails() {
        let mut bits = Bits::new(&[0xFF]);
        for _ in 0..8 {
            bits.next().unwrap();
        }
        assert!(bits.next().is_err());
    }
}
