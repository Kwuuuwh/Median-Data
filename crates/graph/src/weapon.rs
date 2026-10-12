use consensus::Resolved;
use serde::Deserialize;

/// A damage type, in the order DE's `damagePerShot` lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Damage {
    Impact,
    Puncture,
    Slash,
    Heat,
    Cold,
    Electricity,
    Toxin,
    Blast,
    Radiation,
    Gas,
    Magnetic,
    Viral,
    Corrosive,
    Void,
    Tau,
    Cinematic,
    ShieldDrain,
    HealthDrain,
    EnergyDrain,
    Finisher,
}

impl Damage {
    /// Every type, at its `damagePerShot` index.
    pub const ALL: [Damage; 20] = [
        Damage::Impact,
        Damage::Puncture,
        Damage::Slash,
        Damage::Heat,
        Damage::Cold,
        Damage::Electricity,
        Damage::Toxin,
        Damage::Blast,
        Damage::Radiation,
        Damage::Gas,
        Damage::Magnetic,
        Damage::Viral,
        Damage::Corrosive,
        Damage::Void,
        Damage::Tau,
        Damage::Cinematic,
        Damage::ShieldDrain,
        Damage::HealthDrain,
        Damage::EnergyDrain,
        Damage::Finisher,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Damage::Impact => "impact",
            Damage::Puncture => "puncture",
            Damage::Slash => "slash",
            Damage::Heat => "heat",
            Damage::Cold => "cold",
            Damage::Electricity => "electricity",
            Damage::Toxin => "toxin",
            Damage::Blast => "blast",
            Damage::Radiation => "radiation",
            Damage::Gas => "gas",
            Damage::Magnetic => "magnetic",
            Damage::Viral => "viral",
            Damage::Corrosive => "corrosive",
            Damage::Void => "void",
            Damage::Tau => "tau",
            Damage::Cinematic => "cinematic",
            Damage::ShieldDrain => "shield_drain",
            Damage::HealthDrain => "health_drain",
            Damage::EnergyDrain => "energy_drain",
            Damage::Finisher => "finisher",
        }
    }

    /// The type a wiki damage table names.
    pub fn from_wiki(key: &str) -> Option<Damage> {
        Damage::ALL.into_iter().find(|d| d.wiki_key() == key)
    }

    fn wiki_key(self) -> &'static str {
        match self {
            Damage::ShieldDrain => "ShieldDrain",
            Damage::HealthDrain => "HealthDrain",
            Damage::EnergyDrain => "EnergyDrain",
            Damage::Impact => "Impact",
            Damage::Puncture => "Puncture",
            Damage::Slash => "Slash",
            Damage::Heat => "Heat",
            Damage::Cold => "Cold",
            Damage::Electricity => "Electricity",
            Damage::Toxin => "Toxin",
            Damage::Blast => "Blast",
            Damage::Radiation => "Radiation",
            Damage::Gas => "Gas",
            Damage::Magnetic => "Magnetic",
            Damage::Viral => "Viral",
            Damage::Corrosive => "Corrosive",
            Damage::Void => "Void",
            Damage::Tau => "Tau",
            Damage::Cinematic => "Cinematic",
            Damage::Finisher => "Finisher",
        }
    }
}

/// What DE's export says about a weapon or a weapon part.
#[derive(Debug, Clone, PartialEq)]
pub struct Weapon {
    /// `omegaAttenuation`: the riven disposition.
    pub disposition: f64,
    /// `primeOmegaAttenuation`: a kitgun chamber's disposition when built as a primary.
    pub disposition_primary: Option<f64>,
    /// None for a part, which deals no damage of its own.
    pub stats: Option<WeaponStats>,
    /// The weapon as the wiki describes it, once per way it is wielded.
    pub mounts: Vec<Mount>,
    /// None for a weapon no riven fits.
    pub riven: Option<Riven>,
}

/// The riven a weapon takes.
#[derive(Debug, Clone, PartialEq)]
pub struct Riven {
    pub class: Resolved<RivenClass>,
    /// The wiki's weapon line; its variants share one riven.
    pub family: Option<String>,
    /// warframe.market riven slug of the weapon line, the key of its auction search.
    pub slug: Option<String>,
    /// warframe.market's disposition, where its riven entry names this very weapon.
    pub market_disposition: Option<f64>,
}

/// Which riven pool a weapon rolls from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RivenClass {
    Rifle,
    Shotgun,
    Pistol,
    Melee,
    Archgun,
    Kitgun,
    Zaw,
}

impl RivenClass {
    pub fn as_str(self) -> &'static str {
        match self {
            RivenClass::Rifle => "rifle",
            RivenClass::Shotgun => "shotgun",
            RivenClass::Pistol => "pistol",
            RivenClass::Melee => "melee",
            RivenClass::Archgun => "archgun",
            RivenClass::Kitgun => "kitgun",
            RivenClass::Zaw => "zaw",
        }
    }
}

impl std::fmt::Display for RivenClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The one attack DE's export prints for a weapon.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponStats {
    /// `trigger` as DE prints it; melee has none.
    pub trigger: Option<String>,
    pub crit_chance: f64,
    pub crit_multiplier: f64,
    pub status_chance: f64,
    /// Shots per second; attacks per second for melee.
    pub fire_rate: f64,
    pub multishot: Option<i64>,
    pub magazine: Option<i64>,
    pub reload_secs: Option<f64>,
    pub total_damage: f64,
    /// Damage per shot by type, nonzero types only.
    pub damage: Vec<(Damage, f64)>,
}

/// One wiki entry for a weapon. A kitgun chamber has one per slot it is built into, an
/// archgun one for space and one for the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct Mount {
    /// The wiki's entry name, e.g. `Catchmoon (Primary)`.
    pub name: String,
    pub slot: String,
    pub class: String,
    /// The weapon line its variants share, e.g. `Zarr` for Kuva Zarr.
    pub family: Option<String>,
    /// e.g. `Prime`, `Kuva Lich`, `Kitgun`.
    pub traits: Vec<String>,
    pub disposition: Option<f64>,
    /// None where the wiki gives none or calls it unlimited.
    pub ammo_max: Option<i64>,
    /// `AttackIndex` of the attack the wiki shows in the weapon's tooltip.
    pub tooltip_attack: Option<i64>,
    pub attacks: Vec<Attack>,
}

/// One attack of a weapon, as the wiki lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Attack {
    pub index: i64,
    pub name: String,
    /// `AttackIndex` of the attack this one follows from, e.g. a projectile's explosion.
    pub parent: Option<i64>,
    pub shot: Option<Shot>,
    pub trigger: Option<String>,
    pub crit_chance: f64,
    pub crit_multiplier: f64,
    pub status_chance: f64,
    pub fire_rate: f64,
    pub multishot: Option<f64>,
    pub charge_secs: Option<f64>,
    pub ammo_cost: Option<f64>,
    pub punch_through: Option<f64>,
    /// Reach of a hit-scan or a beam; radius of an area attack.
    pub range: Option<f64>,
    pub falloff: Option<Falloff>,
    /// Charge the Incarnon form costs; set only on that form's attacks.
    pub incarnon_charges: Option<i64>,
    /// Damage by type, nonzero types only.
    pub damage: Vec<(Damage, f64)>,
}

/// How an attack delivers its damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shot {
    HitScan,
    Projectile,
    Area,
    OverTime,
    Thrown,
}

impl Shot {
    /// The kind a wiki `ShotType` names.
    pub fn from_wiki(printed: &str) -> Option<Shot> {
        match printed {
            "Hit-Scan" | "Hitscan" => Some(Shot::HitScan),
            "Projectile" => Some(Shot::Projectile),
            "AoE" => Some(Shot::Area),
            "DoT" => Some(Shot::OverTime),
            "Thrown" => Some(Shot::Thrown),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Shot::HitScan => "hitscan",
            Shot::Projectile => "projectile",
            Shot::Area => "area",
            Shot::OverTime => "over_time",
            Shot::Thrown => "thrown",
        }
    }
}

/// How damage falls off with distance: full up to `start_m`, down by `reduction` at `end_m`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Falloff {
    pub start_m: f64,
    pub end_m: f64,
    pub reduction: f64,
}

/// The stats one class of riven rolls from.
#[derive(Debug, Clone, PartialEq)]
pub struct RivenPool {
    pub class: RivenClass,
    pub stats: Vec<RivenStat>,
    /// What a veiled riven of this class can ask before it unveils.
    pub challenges: Vec<RivenChallenge>,
    /// Pairs of its stats a Riven Splicer turns into a third.
    pub splices: Vec<Splice>,
}

/// Two riven stats a Riven Splicer consumes, and the stat it gives for them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Splice {
    pub first: String,
    pub second: String,
    pub result: String,
}

/// A task a veiled riven asks, e.g. `Kill |COUNT| Enemies with Headshots`.
#[derive(Debug, Clone, PartialEq)]
pub struct RivenChallenge {
    /// DE's path, as the riven's fingerprint names it.
    pub path: String,
    pub shown: Shown,
    /// The wording for a count of one, where the client has its own.
    pub shown_single: Option<Shown>,
    pub complications: Vec<Complication>,
}

/// A condition a challenge can come with, e.g. `while sliding`.
#[derive(Debug, Clone, PartialEq)]
pub struct Complication {
    pub path: String,
    pub shown: Shown,
}

/// A line the client prints, in both languages.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub en: String,
    pub ru: Option<String>,
}

/// One stat a riven can roll.
#[derive(Debug, Clone, PartialEq)]
pub struct RivenStat {
    /// DE's key for the stat, e.g. `WeaponCritDamageMod`; GEP prints it as `Tag`.
    pub tag: String,
    /// The name parts a riven built on this stat takes, e.g. `acri` and `tis`.
    pub prefix: NamePart,
    pub suffix: NamePart,
    /// `upgradeValues[].value`: the stat at rank 8 is base × 90 × disposition × layout × roll.
    pub bases: Vec<f64>,
    /// DE flips the sign the client prints.
    pub reverse_symbol: bool,
    pub form: StatForm,
    /// How the client prints the stat, `|val|` standing for the number.
    pub shown_en: String,
    pub shown_ru: Option<String>,
    pub market: Option<MarketStat>,
}

/// A riven name part in both languages.
#[derive(Debug, Clone, PartialEq)]
pub struct NamePart {
    pub en: String,
    pub ru: Option<String>,
}

/// How the client prints a riven stat's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatForm {
    /// `+105.5%`.
    Percent,
    /// `x1.58`: damage to a faction.
    Multiplier,
    /// `+2.2`: punch through, range, seconds.
    Plain,
}

impl StatForm {
    pub fn as_str(self) -> &'static str {
        match self {
            StatForm::Percent => "percent",
            StatForm::Multiplier => "multiplier",
            StatForm::Plain => "plain",
        }
    }
}

/// What warframe.market says about a riven stat.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketStat {
    /// The attribute's slug in riven auctions, e.g. `critical_damage`.
    pub slug: String,
    pub positive_only: bool,
    pub negative_only: bool,
    /// A rise of the number is a loss, as with recoil.
    pub positive_is_negative: bool,
    /// Riven classes the stat rolls on; empty for all.
    pub exclusive_to: Vec<String>,
}
