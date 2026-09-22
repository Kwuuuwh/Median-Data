use serde::Serialize;
use sources::notation::{Block, Value};

/// What one manifest sells, as the game wrote it.
#[derive(Debug, Serialize, PartialEq)]
pub struct Stall {
    pub manifest: String,
    /// The game rolls this stall's prices, so what it lists is a range and not a price.
    #[serde(skip_serializing_if = "is_false")]
    pub floating: bool,
    pub offer: Vec<Offer>,
}

/// One thing a stall hands over, and what it asks for it.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Offer {
    pub item: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<[i64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platinum: Option<[i64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standing: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    /// Hours the offer stays up, where it rotates on a timer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<[i64; 2]>,
    /// The counter the offer stands on, where the vendor keeps more than one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<String>,
    /// The pool the offer rotates within.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affiliation: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub always: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub weekly: bool,
    /// Items the offer is paid in, where it is paid in items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub price: Vec<Price>,
}

/// One item a price is counted in.
#[derive(Debug, Serialize, PartialEq)]
pub struct Price {
    pub item: String,
    pub count: i64,
}

/// Read what a manifest sells, whichever of the two schemas it is written in.
pub fn stall(manifest: &str, written: &Block) -> Stall {
    let mut offer = Vec::new();
    if let Some(listed) = written.block("ItemManifest") {
        offer.extend(blocks(listed).map(sold));
    }
    if let Some(listed) = written.block("Favors") {
        offer.extend(blocks(listed).map(favour));
    }
    Stall {
        manifest: manifest.to_string(),
        floating: written.int("IsDynamic") == Some(1),
        offer,
    }
}

/// An offer of the vendor schema, which prices in items, credits and platinum.
fn sold(written: &Block) -> Offer {
    Offer {
        item: written.text("StoreItem").unwrap_or_default().to_string(),
        count: written
            .int("QuantityMultiplier")
            .filter(|count| *count != 1),
        credits: span(written, "RegularPrice"),
        platinum: span(written, "PremiumPrice"),
        standing: written.int("StandingCost").filter(|cost| *cost > 0),
        rank: written.int("MinAffiliationRank").filter(|rank| *rank > 0),
        limit: written
            .int("PurchaseQuantityLimit")
            .filter(|limit| *limit > 0),
        hours: span(written, "DurationAvailable"),
        store: None,
        bin: written.text("Bin").map(str::to_string),
        affiliation: text(written, "Affiliation"),
        always: written.int("AlwaysOffered") == Some(1),
        weekly: written.int("RotatedWeekly") == Some(1),
        price: prices(written),
    }
}

/// An offer of the syndicate schema, which prices in standing and credits.
fn favour(written: &Block) -> Offer {
    Offer {
        item: written.text("storeItem").unwrap_or_default().to_string(),
        standing: written.int("standingCost").filter(|cost| *cost > 0),
        credits: written
            .int("creditsCost")
            .filter(|cost| *cost > 0)
            .map(|cost| [cost, cost]),
        rank: written.int("requiredLevel").filter(|rank| *rank > 0),
        limit: written.int("purchaseLimit").filter(|limit| *limit > 0),
        store: text(written, "availabilityTag"),
        ..Offer::default()
    }
}

fn prices(written: &Block) -> Vec<Price> {
    let Some(listed) = written.block("ItemPrices") else {
        return Vec::new();
    };
    blocks(listed)
        .filter_map(|price| {
            Some(Price {
                item: price.text("ItemType")?.to_string(),
                count: price.int("ItemCount")?,
            })
        })
        .collect()
}

/// A pair the game writes as `{low,high}`, where it is not plain zero.
fn span(written: &Block, key: &str) -> Option<[i64; 2]> {
    let listed = written.block(key)?;
    let read = |at: usize| listed.items.get(at)?.text()?.parse::<i64>().ok();
    let (low, high) = (read(0)?, read(1)?);
    (low != 0 || high != 0).then_some([low, high])
}

fn text(written: &Block, key: &str) -> Option<String> {
    written
        .text(key)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn blocks(listed: &Block) -> impl Iterator<Item = &Block> {
    listed.items.iter().filter_map(Value::block)
}

fn is_false(flag: &bool) -> bool {
    !flag
}

#[cfg(test)]
mod tests {
    use super::*;
    use sources::notation;

    fn manifest(text: &str) -> Block {
        let mut raw = vec![0u8; 20];
        raw.extend_from_slice(&(text.len() as u32).to_le_bytes());
        raw.extend_from_slice(text.as_bytes());
        notation::parse(&raw).expect("a manifest")
    }

    #[test]
    fn a_vendor_offer_carries_its_price_in_items() {
        let written = manifest(
            "IsDynamic=1
ItemManifest={
    {
        StoreItem=/Lotus/StoreItems/Types/Recipes/OperatorTeshinArmsBlueprint
        QuantityMultiplier=1
        AlwaysOffered=1
        Bin=BIN_0
        DurationAvailable={24,24}
        PurchaseQuantityLimit=0
        RegularPrice={0,0}
        PremiumPrice={5,10}
        ItemPrices={
            {
                ItemCount=15
                ItemType=/Lotus/Types/Items/MiscItems/SteelEssence
            }
        }
        Affiliation=\"\"
        MinAffiliationRank=0
    }
}
",
        );

        let stall = stall(
            "/Lotus/Types/Game/VendorManifests/Hubs/TeshinManifest",
            &written,
        );

        assert!(stall.floating);
        assert_eq!(stall.offer.len(), 1);
        let offer = &stall.offer[0];
        assert_eq!(
            offer.item,
            "/Lotus/StoreItems/Types/Recipes/OperatorTeshinArmsBlueprint"
        );
        assert_eq!(
            offer.price,
            [Price {
                item: "/Lotus/Types/Items/MiscItems/SteelEssence".to_string(),
                count: 15,
            }]
        );
        assert_eq!(offer.platinum, Some([5, 10]));
        assert_eq!(offer.credits, None);
        assert_eq!(offer.hours, Some([24, 24]));
        assert_eq!(offer.bin.as_deref(), Some("BIN_0"));
        assert!(offer.always);
        assert_eq!(offer.count, None);
        assert_eq!(offer.limit, None);
        assert_eq!(offer.rank, None);
        assert_eq!(offer.affiliation, None);
    }

    #[test]
    fn a_syndicate_favour_carries_standing_and_a_counter() {
        let written = manifest(
            "Favors={
    {
        storeItem=/Lotus/StoreItems/Weapons/Ostron/Melee/BalanceDamageIBlueprint
        standingCost=1000
        creditsCost=3500
        requiredLevel=2
        availabilityTag=Weaponsmith
        purchaseLimit=0
    }
}
",
        );

        let stall = stall("/Lotus/Syndicates/Ostron/CetusManifest", &written);

        assert!(!stall.floating);
        let offer = &stall.offer[0];
        assert_eq!(offer.standing, Some(1000));
        assert_eq!(offer.credits, Some([3500, 3500]));
        assert_eq!(offer.rank, Some(2));
        assert_eq!(offer.store.as_deref(), Some("Weaponsmith"));
        assert_eq!(offer.limit, None);
        assert!(offer.price.is_empty());
    }

    #[test]
    fn a_price_of_several_tags_keeps_them_all() {
        let written = manifest(
            "ItemManifest={
    {
        StoreItem=/Lotus/StoreItems/Upgrades/Skins/Clan/ConservationBadgeDeimosItem
        ItemPrices={
            {
                ItemCount=1
                ItemType=/Lotus/Types/Items/Deimos/AnimalTagInfestedMaggotCommon
            },
            {
                ItemCount=2
                ItemType=/Lotus/Types/Items/Deimos/AnimalTagInfestedMaggotRare
            }
        }
    }
}
",
        );

        let stall = stall(
            "/Lotus/Types/Game/VendorManifests/Deimos/ConservationManifest",
            &written,
        );

        assert_eq!(stall.offer[0].price.len(), 2);
        assert_eq!(stall.offer[0].price[1].count, 2);
    }
}
