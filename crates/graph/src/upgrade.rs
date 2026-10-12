/// A weapon mod: what it fits, what it costs, and what it does at its top rank.
#[derive(Debug, Clone, PartialEq)]
pub struct ModInfo {
    /// `compatName` as DE prints it in English: a weapon class, or a weapon for an augment.
    pub compat: String,
    pub rarity: String,
    pub polarity: String,
    pub base_drain: i64,
    pub max_rank: i64,
    /// Fits one weapon only.
    pub augment: bool,
    /// A Conclave copy.
    pub pvp: bool,
    pub exilus: bool,
    pub primed: bool,
    pub umbral: bool,
    /// The set the mod counts toward, where it is part of one.
    pub mod_set: Option<String>,
    pub top_rank: RankText,
}

/// An arcane and what it does at its top rank.
#[derive(Debug, Clone, PartialEq)]
pub struct ArcaneInfo {
    pub rarity: Option<String>,
    pub max_rank: i64,
    pub top_rank: RankText,
}

/// What a mod or an arcane says it does at one rank: the lines as the client prints them,
/// and the effects read out of them.
#[derive(Debug, Clone, PartialEq)]
pub struct RankText {
    pub lines: Vec<Line>,
    pub effects: Vec<Effect>,
}

/// One printed line, in both languages.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub en: String,
    pub ru: Option<String>,
}

/// One effect: a stat moved by an amount, maybe only under a condition and for a while.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// Our key for the stat, e.g. `multishot`, `crit_chance`, `faction_grineer`.
    pub stat: String,
    /// As printed: `165` for `+165%`, `2.2` for `+2.2 Punch Through`, `1.3` for `x1.3`.
    pub amount: f64,
    pub unit: Unit,
    /// What it waits for, as printed, e.g. `On Kill`, `When Aiming`.
    pub condition: Option<String>,
    /// Chance in percent that the condition grants it.
    pub chance: Option<f64>,
    pub duration_secs: Option<f64>,
    pub max_stacks: Option<i64>,
}

/// How an effect's amount reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Plain,
    Multiplier,
    Meters,
    Seconds,
}

impl Unit {
    pub fn as_str(self) -> &'static str {
        match self {
            Unit::Percent => "percent",
            Unit::Plain => "plain",
            Unit::Multiplier => "multiplier",
            Unit::Meters => "meters",
            Unit::Seconds => "seconds",
        }
    }
}
