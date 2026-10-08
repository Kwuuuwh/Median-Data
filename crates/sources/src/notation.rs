use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};

/// Hash, a zero and the text length, ahead of the text itself.
const HEADER_LEN: usize = 24;

/// A value of a manifest: plain text, or a block.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Block(Block),
}

/// A manifest block: positional items in order, named fields by key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Block {
    pub items: Vec<Value>,
    pub fields: BTreeMap<String, Value>,
}

impl Block {
    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        self.field(key)?.text()
    }

    pub fn block(&self, key: &str) -> Option<&Block> {
        self.field(key)?.block()
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.text(key)?.parse().ok()
    }
}

impl Value {
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            Value::Block(_) => None,
        }
    }

    pub fn block(&self) -> Option<&Block> {
        match self {
            Value::Block(block) => Some(block),
            Value::Text(_) => None,
        }
    }
}

/// Read a manifest the game wrote as text behind a short binary header.
pub fn parse(raw: &[u8]) -> Result<Block> {
    let len = u32::from_le_bytes(
        raw.get(HEADER_LEN - 4..HEADER_LEN)
            .context("manifest is shorter than its header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let text = raw
        .get(HEADER_LEN..HEADER_LEN + len)
        .context("manifest text runs past the file")?;
    let text = std::str::from_utf8(text).context("manifest text is not UTF-8")?;
    read(text)
}

/// Read text written in the game's notation.
pub fn read(text: &str) -> Result<Block> {
    let mut open: Vec<(Option<String>, Block)> = vec![(None, Block::default())];
    for (no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "{" {
            open.push((None, Block::default()));
            continue;
        }
        if line == "}" || line == "}," {
            if open.len() == 1 {
                bail!("line {}: a block closes that never opened", no + 1);
            }
            let (key, block) = open.pop().unwrap();
            put(&mut open.last_mut().unwrap().1, key, Value::Block(block));
            continue;
        }
        match line.split_once('=') {
            Some((key, "{")) => open.push((Some(key.trim().to_string()), Block::default())),
            Some((key, value)) => {
                let value = match value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
                    Some(inner) => Value::Block(row(inner)),
                    None => Value::Text(unquote(value).to_string()),
                };
                put(
                    &mut open.last_mut().unwrap().1,
                    Some(key.trim().to_string()),
                    value,
                );
            }
            None => {
                let item = Value::Text(unquote(line.trim_end_matches(',')).to_string());
                put(&mut open.last_mut().unwrap().1, None, item);
            }
        }
    }
    if open.len() != 1 {
        bail!("{} blocks were left open", open.len() - 1);
    }
    Ok(open.pop().unwrap().1)
}

fn put(block: &mut Block, key: Option<String>, value: Value) {
    match key {
        Some(key) => {
            block.fields.insert(key, value);
        }
        None => block.items.push(value),
    }
}

/// A block written on one line, such as `{24,24}`.
fn row(inner: &str) -> Block {
    let mut block = Block::default();
    for part in inner.split(',') {
        let part = part.trim();
        if !part.is_empty() {
            block.items.push(Value::Text(unquote(part).to_string()));
        }
    }
    block
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CETUS: &str = "\
Favors={
    {
        storeItem=/Lotus/StoreItems/Weapons/Ostron/Melee/ModularMelee01/Balance/BalanceDamageIBlueprint
        standingCost=1000
        creditsCost=0
        requiredLevel=0
        availableAsFreeFavor=1
        availabilityTag=Weaponsmith
        questReward=0
        availabilityScript={
            Script=\"\"
        }
        purchaseLimit=0
    }
}
";

    const ACRITHIS: &str = "\
IsDynamic=1
ItemManifest={
    {
        StoreItem=/Lotus/StoreItems/Types/Items/MiscItems/Ferrite
        Bin=BIN_0
        DurationAvailable={1,3}
        RegularPrice={0,0}
        ItemPrices={}
        Affiliation=\"\"
        FocusXpCost={
            Polarity=AP_UNIVERSAL
            Cost=0
        }
        PurchaseQuantityLimit=1
    }
}
NumItemsPerBin={
    BIN_0=5
    BIN_1=1
}
";

    fn manifest(text: &str) -> Vec<u8> {
        let mut raw = vec![0u8; 20];
        raw.extend_from_slice(&(text.len() as u32).to_le_bytes());
        raw.extend_from_slice(text.as_bytes());
        raw.extend_from_slice(&[0u8; 9]);
        raw
    }

    #[test]
    fn a_syndicate_manifest_lists_its_favors() {
        let cetus = parse(&manifest(CETUS)).unwrap();
        let favors = cetus.block("Favors").unwrap();
        assert_eq!(favors.items.len(), 1);

        let favor = favors.items[0].block().unwrap();
        assert_eq!(
            favor.text("storeItem"),
            Some(
                "/Lotus/StoreItems/Weapons/Ostron/Melee/ModularMelee01/Balance/BalanceDamageIBlueprint"
            )
        );
        assert_eq!(favor.text("standingCost"), Some("1000"));
        assert_eq!(favor.text("availabilityTag"), Some("Weaponsmith"));
        assert_eq!(
            favor.block("availabilityScript").unwrap().text("Script"),
            Some("")
        );
    }

    #[test]
    fn a_vendor_manifest_lists_its_offers() {
        let acrithis = parse(&manifest(ACRITHIS)).unwrap();
        assert_eq!(acrithis.text("IsDynamic"), Some("1"));

        let offer = acrithis.block("ItemManifest").unwrap().items[0]
            .block()
            .unwrap();
        assert_eq!(
            offer.text("StoreItem"),
            Some("/Lotus/StoreItems/Types/Items/MiscItems/Ferrite")
        );
        assert_eq!(offer.text("Affiliation"), Some(""));
        assert_eq!(
            offer.block("FocusXpCost").unwrap().text("Polarity"),
            Some("AP_UNIVERSAL")
        );

        let duration = offer.block("DurationAvailable").unwrap();
        assert_eq!(
            duration.items,
            vec![Value::Text("1".into()), Value::Text("3".into())]
        );
        assert_eq!(offer.block("ItemPrices"), Some(&Block::default()));

        let bins = acrithis.block("NumItemsPerBin").unwrap();
        assert_eq!(bins.text("BIN_0"), Some("5"));
    }

    #[test]
    fn a_number_reads_as_one() {
        let acrithis = parse(&manifest(ACRITHIS)).unwrap();
        let offer = acrithis.block("ItemManifest").unwrap().items[0]
            .block()
            .unwrap();

        assert_eq!(offer.int("PurchaseQuantityLimit"), Some(1));
        assert_eq!(offer.int("StoreItem"), None);
    }

    #[test]
    fn numbers_on_their_own_lines_keep_their_order() {
        let schedule = parse(&manifest("Cycles={\n    9,\n    32,\n    35\n}\n")).unwrap();
        let cycles = schedule.block("Cycles").unwrap();
        assert_eq!(
            cycles.items,
            vec![
                Value::Text("9".into()),
                Value::Text("32".into()),
                Value::Text("35".into())
            ]
        );
    }

    #[test]
    fn a_file_that_is_not_text_is_refused() {
        let raw = [0xC9u8; 257];
        assert!(parse(&raw).is_err());
    }

    #[test]
    fn a_stray_closing_brace_is_refused() {
        let err = parse(&manifest("Favors={\n}\n}\n")).unwrap_err();
        assert!(err.to_string().contains("never opened"));
    }
}
