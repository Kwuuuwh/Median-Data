/// How an item drops from a place.
#[derive(Debug, Clone, PartialEq)]
pub struct DropInfo {
    pub rarity: String,
    /// Probability within the place's table.
    pub chance: f64,
    pub rotation: Option<String>,
    pub stage: Option<String>,
    /// An enemy's chance to roll that table at all.
    pub table_chance: Option<f64>,
}

/// What a vendor asks for an item, and how steadily they offer it. The cost is in the
/// vendor's own currency, which the vendor node names — standing, ducats, platinum, or one of
/// the thirty tokens the game has accumulated.
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub cost: Option<i64>,
    /// What the cost is in. It belongs to the offer, not the vendor: one person can keep
    /// several counters, each taking a different token.
    pub currency: Option<String>,
    /// The counter the offer stands on, where the vendor keeps more than one.
    pub store: Option<String>,
    /// Some vendors charge credits on top of their own currency.
    pub credits: Option<i64>,
    /// How many the offer hands over.
    pub count: i64,
    /// Standing rank the vendor requires, where they have ranks.
    pub rank: Option<i64>,
    /// Seconds the offer stays up, where it rotates on a timer.
    pub timer: Option<i64>,
    /// How many visits it has been offered on, where the source counts them.
    pub times: usize,
    /// Offered every time rather than in rotation.
    pub always: bool,
    /// Offered once but no longer.
    pub gone: bool,
}

/// What a clan pays to research something in a dojo lab. The research unlocks the blueprint;
/// building the thing then costs what its own recipe says, which is why this is a separate
/// price and not part of the foundry cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Research {
    pub credits: i64,
    /// Seconds the research takes.
    pub time: i64,
    /// Mastery the research grants.
    pub affinity: i64,
    /// What has to be researched first, by its printed name.
    pub prereq: Option<String>,
    /// Materials the clan hands in, as name and count.
    pub resources: Vec<(String, i64)>,
}

/// A typed relation between two nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Rel {
    /// Recipe -> the item it assembles.
    Produces,
    /// Recipe -> an ingredient it consumes.
    Requires { count: i64 },
    /// Relic -> an item it can award.
    Rewards { rarity: String },
    /// Set -> a part belonging to it.
    Member,
    /// Set -> the assembled item it stands for.
    Represents,
    /// Place -> an item it can drop.
    Drops(DropInfo),
    /// An ordinary item -> its prime counterpart.
    Primed,
    /// An imprint -> the animal it breeds.
    Yields,
    /// A place -> the star-chart node it names.
    At,
    /// A vendor -> an item they hand over for currency.
    Sells(Offer),
    /// A relic -> the next refinement of itself. Only the intact one drops; the other three
    /// are made from it with void traces, so this is how they are obtained at all.
    Refines,
    /// A dojo lab -> the blueprint its research unlocks.
    Researched(Research),
}

impl Rel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Rel::Produces => "produces",
            Rel::Requires { .. } => "requires",
            Rel::Rewards { .. } => "rewards",
            Rel::Member => "member",
            Rel::Represents => "represents",
            Rel::Drops(_) => "drops",
            Rel::Primed => "primed",
            Rel::Yields => "yields",
            Rel::At => "at",
            Rel::Sells(_) => "sells",
            Rel::Refines => "refines",
            Rel::Researched(_) => "researched",
        }
    }
}

/// A directed, typed link between two node ids.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub rel: Rel,
}
