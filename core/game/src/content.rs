//! The content of the game as data: relics, upgrades, floors, prices, and constants.
//!
//! - Relic: add one entry to `RELICS`. If the entry has a `foe_text`, a boss can have it as a trait.
//! - Relic effect at a new point of a battle: add a kind to `Effect`, and apply it in `battle.rs`.
//! - Movement rule: add a kind to `RuleEdit` if no kind gives it. The AI reads the rules data.
//! - Upgrade: add one entry to `UPGRADES`. A new kind of effect needs a kind in `UpgradeEffect`.
//! - Floor: add one entry to `FLOORS`.

use crate::chess::{
    ALFIL, Atom, CAMEL, DABBABA, DIAG, FORWARD, FORWARD_DIAG, KING, KNIGHT, Kind, Mode, ORTHO, Offset, SideRules,
};

/// An edit of `SideRules::standard()` that a relic gives to its side. The engine builds its
/// tables and the AI its piece values from the result, thus the AI sees each relic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuleEdit {
    /// Pawns can always move two squares forward.
    ForcedMarch,
    /// Pawns promote one rank earlier.
    EarlyPromo,
    /// Pawns can move one square backward to an empty square.
    Backpedal,
    /// The kind can also jump by each offset.
    Leap { kind: Kind, offsets: &'static [Offset], mode: Mode },
    /// The kind can also go 1 to `steps` steps along each offset. Each step before the last one
    /// must end on an empty square.
    Slide { kind: Kind, offsets: &'static [Offset], mode: Mode, steps: u8 },
}

impl RuleEdit {
    pub fn apply(self, rules: SideRules) -> SideRules {
        match self {
            RuleEdit::ForcedMarch => rules.forced_march(),
            RuleEdit::EarlyPromo => rules.early_promo(),
            RuleEdit::Backpedal => rules.backpedal(),
            RuleEdit::Leap { kind, offsets, mode } => rules.with_atom(kind, Atom::leap(offsets, mode)),
            RuleEdit::Slide { kind, offsets, mode, steps } => {
                rules.with_atom(kind, Atom::slide(offsets, mode).max_steps(steps))
            }
        }
    }
}

/// What a relic does at a fixed point of a battle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// The gold of each capture by the player is multiplied by `factor`.
    CaptureGold { factor: f64 },
    /// The first unit that the player loses in a battle returns after the battle.
    RescueFirst,
    /// One more pawn on the first free square of rank 2, or of rank 3. It is not a unit.
    ExtraPawn,
    /// After a win: 1 gold for each `per` gold of the run and the other rewards, at most `max`.
    VictoryGold { per: u64, max: u64 },
    /// After a win: one more pawn in the army, if a pawn of the army promoted in the battle and is
    /// on the board.
    PromotionRecruit,
    /// After a win by checkmate: the gold value of each enemy piece on the board.
    CheckmateGold,
    /// After a win or a draw: half of the shop price of each piece that the enemy captured.
    LossGold,
}

pub struct RelicDef {
    /// The id in the saved data and in the protocol.
    pub key: &'static str,
    pub name: &'static str,
    pub text: &'static str,
    /// The text when the enemy has the relic. A relic with this text can be a boss trait.
    pub foe_text: Option<&'static str>,
    pub rules: &'static [RuleEdit],
    pub effect: Option<Effect>,
}

const fn rule(
    key: &'static str,
    name: &'static str,
    text: &'static str,
    foe: &'static str,
    rules: &'static [RuleEdit],
) -> RelicDef {
    RelicDef { key, name, text, foe_text: Some(foe), rules, effect: None }
}

const fn hook(key: &'static str, name: &'static str, text: &'static str, effect: Effect) -> RelicDef {
    RelicDef { key, name, text, foe_text: None, rules: &[], effect: Some(effect) }
}

pub static RELICS: [RelicDef; 24] = [
    rule(
        "forcedMarch",
        "Forced March",
        "Your pawns can always move two squares forward.",
        "Enemy pawns can always move two squares forward.",
        &[RuleEdit::ForcedMarch],
    ),
    rule(
        "backpedal",
        "Tactical Retreat",
        "Your pawns can move one square backward to an empty square.",
        "Enemy pawns can move one square backward to an empty square.",
        &[RuleEdit::Backpedal],
    ),
    rule(
        "earlyPromo",
        "Field Promotion",
        "Your pawns promote one rank earlier.",
        "Enemy pawns promote one rank earlier.",
        &[RuleEdit::EarlyPromo],
    ),
    rule(
        "kingKnight",
        "Royal Steed",
        "Your king can also move as a knight.",
        "The enemy king can also move as a knight.",
        &[RuleEdit::Leap { kind: Kind::King, offsets: &KNIGHT, mode: Mode::MoveOrCapture }],
    ),
    rule(
        "longLeap",
        "Long Leap",
        "Your knights can also jump three squares in one direction and one square to the side.",
        "Enemy knights can also jump three squares in one direction and one square to the side.",
        &[RuleEdit::Leap { kind: Kind::Knight, offsets: &CAMEL, mode: Mode::MoveOrCapture }],
    ),
    rule(
        "sidestep",
        "Sidestep",
        "Your bishops can move one square up, down, left, or right to an empty square.",
        "Enemy bishops can move one square up, down, left, or right to an empty square.",
        &[RuleEdit::Leap { kind: Kind::Bishop, offsets: &ORTHO, mode: Mode::MoveOnly }],
    ),
    hook("bounty", "Bounty", "You get 50% more gold for each capture.", Effect::CaptureGold { factor: 1.5 }),
    hook(
        "secondWind",
        "Second Wind",
        "The first piece that you lose in each battle returns after the battle.",
        Effect::RescueFirst,
    ),
    hook(
        "conscription",
        "Conscription",
        "You start each battle with one more pawn. The pawn leaves after the battle.",
        Effect::ExtraPawn,
    ),
    hook(
        "interest",
        "Interest",
        "After each battle that you win, you get 1 gold for each 5 gold that you have. The maximum is 6 gold.",
        Effect::VictoryGold { per: 5, max: 6 },
    ),
    rule(
        "vault",
        "Rampart Vault",
        "Your rooks can jump two squares up, down, left, or right to an empty square. A piece between does not stop the jump.",
        "Enemy rooks can jump two squares up, down, left, or right to an empty square. A piece between does not stop the jump.",
        &[RuleEdit::Leap { kind: Kind::Rook, offsets: &DABBABA, mode: Mode::MoveOnly }],
    ),
    rule(
        "crossfire",
        "Crossfire",
        "Your rooks can capture a piece that is one square away diagonally.",
        "Enemy rooks can capture a piece that is one square away diagonally.",
        &[RuleEdit::Leap { kind: Kind::Rook, offsets: &DIAG, mode: Mode::CaptureOnly }],
    ),
    rule(
        "closeQuarters",
        "Close Quarters",
        "Your knights can capture a piece that is one square up, down, left, or right.",
        "Enemy knights can capture a piece that is one square up, down, left, or right.",
        &[RuleEdit::Leap { kind: Kind::Knight, offsets: &ORTHO, mode: Mode::CaptureOnly }],
    ),
    rule(
        "pilgrimLeap",
        "Pilgrim's Leap",
        "Your bishops can jump two squares diagonally. A piece between does not stop the jump.",
        "Enemy bishops can jump two squares diagonally. A piece between does not stop the jump.",
        &[RuleEdit::Leap { kind: Kind::Bishop, offsets: &ALFIL, mode: Mode::MoveOrCapture }],
    ),
    rule(
        "queenFlight",
        "Queen's Flight",
        "Your queens can jump as a knight to an empty square.",
        "Enemy queens can jump as a knight to an empty square.",
        &[RuleEdit::Leap { kind: Kind::Queen, offsets: &KNIGHT, mode: Mode::MoveOnly }],
    ),
    rule(
        "gallop",
        "Gallop",
        "Your knights can make a second jump in the same direction if the first square is empty.",
        "Enemy knights can make a second jump in the same direction if the first square is empty.",
        &[RuleEdit::Slide { kind: Kind::Knight, offsets: &KNIGHT, mode: Mode::MoveOrCapture, steps: 2 }],
    ),
    rule(
        "crusade",
        "Crusade",
        "Your bishops can also move straight forward, as a rook does.",
        "Enemy bishops can also move straight forward, as a rook does.",
        &[RuleEdit::Slide { kind: Kind::Bishop, offsets: &FORWARD, mode: Mode::MoveOrCapture, steps: Atom::MAX_STEPS }],
    ),
    rule(
        "royalMarch",
        "Royal March",
        "Your king can move two squares in a straight line if the first square is empty.",
        "The enemy king can move two squares in a straight line if the first square is empty.",
        &[RuleEdit::Slide { kind: Kind::King, offsets: &KING, mode: Mode::MoveOrCapture, steps: 2 }],
    ),
    rule(
        "huntress",
        "Huntress",
        "Your queens can capture as a knight.",
        "Enemy queens can capture as a knight.",
        &[RuleEdit::Leap { kind: Kind::Queen, offsets: &KNIGHT, mode: Mode::CaptureOnly }],
    ),
    hook(
        "apprenticeship",
        "Apprenticeship",
        "After each battle that you win, you get a pawn if one of your pawns promoted in the battle and is still on the board.",
        Effect::PromotionRecruit,
    ),
    hook(
        "coup",
        "Coup de Grace",
        "When you win a battle by checkmate, you get the gold value of each enemy piece that is still on the board.",
        Effect::CheckmateGold,
    ),
    hook(
        "gambit",
        "Gambit",
        "When the enemy captures one of your pieces, you get half of its shop price in gold after the battle.",
        Effect::LossGold,
    ),
    rule(
        "shieldWall",
        "Shield Wall",
        "Your pawns can capture a piece that is one square straight forward.",
        "Enemy pawns can capture a piece that is one square straight forward.",
        &[RuleEdit::Leap { kind: Kind::Pawn, offsets: &FORWARD, mode: Mode::CaptureOnly }],
    ),
    rule(
        "echelon",
        "Echelon",
        "Your pawns can move one square diagonally forward to an empty square.",
        "Enemy pawns can move one square diagonally forward to an empty square.",
        &[RuleEdit::Leap { kind: Kind::Pawn, offsets: &FORWARD_DIAG, mode: Mode::MoveOnly }],
    ),
];

/// A relic: an index in `RELICS`. The only ways to get one are `parse` and `all`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RelicId(u8);

impl RelicId {
    pub fn all() -> impl Iterator<Item = RelicId> {
        (0..RELICS.len() as u8).map(RelicId)
    }

    pub fn parse(key: &str) -> Option<RelicId> {
        RelicId::all().find(|id| id.def().key == key)
    }

    pub fn def(self) -> &'static RelicDef {
        &RELICS[self.0 as usize]
    }

    pub fn key(self) -> &'static str {
        self.def().key
    }

    /// True if a boss can have the relic as a trait.
    pub fn is_trait(self) -> bool {
        self.def().foe_text.is_some()
    }
}

/// The movement rules that a list of relics gives to one side. A relic counts one time.
pub fn rules_for(ids: &[RelicId]) -> SideRules {
    let mut seen: Vec<RelicId> = Vec::new();
    let mut rules = SideRules::standard();
    for &id in ids {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        rules = id.def().rules.iter().fold(rules, |rules, edit| edit.apply(rules));
    }
    rules
}

// ---- Upgrades ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpgradeEffect {
    /// A new run has one more unit of the kind for each level.
    RecruitEachLevel(Kind),
    /// A new run has one more unit of the kind.
    Recruit(Kind),
    /// A new run has this gold for each level.
    GoldEachLevel(u64),
    /// Shop prices get the factor `1 - per_level * level`.
    PriceCut { per_level: f64 },
    /// In a battle, the player can see the moves of each enemy piece.
    Scout,
    /// A new run has one random relic.
    StartRelic,
    /// A relic in the shop costs `per_level` gold less for each level, before the factor of `PriceCut`.
    RelicPriceCut { per_level: u64 },
    /// New shop items cost `per_level` gold less for each level.
    RerollCut { per_level: u64 },
    /// A draw gives a reward, as a win does.
    DrawReward,
    /// A unit that the player takes as a reward comes with a pawn.
    RewardPawn,
    /// Each reward has one relic or more.
    DraftRelic,
}

pub struct UpgradeDef {
    pub key: &'static str,
    pub name: &'static str,
    pub text: &'static str,
    /// The crown cost of each level. The number of costs is the maximum level.
    pub costs: &'static [u64],
    pub effect: UpgradeEffect,
}

pub static UPGRADES: [UpgradeDef; 12] = [
    UpgradeDef {
        key: "pawn",
        name: "Militia",
        text: "You start each run with one more pawn for each level.",
        costs: &[3, 5, 8],
        effect: UpgradeEffect::RecruitEachLevel(Kind::Pawn),
    },
    UpgradeDef {
        key: "gold",
        name: "Treasury",
        text: "You start each run with 5 more gold for each level.",
        costs: &[2, 4, 6],
        effect: UpgradeEffect::GoldEachLevel(5),
    },
    UpgradeDef {
        key: "bishop",
        name: "Chaplain",
        text: "You start each run with a bishop.",
        costs: &[6],
        effect: UpgradeEffect::Recruit(Kind::Bishop),
    },
    UpgradeDef {
        key: "haggle",
        name: "Haggler",
        text: "Shop prices decrease by 10% for each level.",
        costs: &[5, 8],
        effect: UpgradeEffect::PriceCut { per_level: 0.1 },
    },
    UpgradeDef {
        key: "scout",
        name: "Scout",
        text: "In a battle, select an enemy piece to see the squares that it can move to.",
        costs: &[4],
        effect: UpgradeEffect::Scout,
    },
    UpgradeDef {
        key: "knight",
        name: "Squire",
        text: "You start each run with a second knight.",
        costs: &[6],
        effect: UpgradeEffect::Recruit(Kind::Knight),
    },
    UpgradeDef {
        key: "heirloom",
        name: "Heirloom",
        text: "You start each run with one random relic.",
        costs: &[8],
        effect: UpgradeEffect::StartRelic,
    },
    UpgradeDef {
        key: "antiquary",
        name: "Antiquary",
        text: "Relics in the shop cost 2 gold less for each level.",
        costs: &[4, 7],
        effect: UpgradeEffect::RelicPriceCut { per_level: 2 },
    },
    UpgradeDef {
        key: "fixer",
        name: "Fixer",
        text: "New shop items cost 1 gold less for each level.",
        costs: &[3, 5],
        effect: UpgradeEffect::RerollCut { per_level: 1 },
    },
    UpgradeDef {
        key: "envoy",
        name: "Envoy",
        text: "After a draw, you select a reward as after a win.",
        costs: &[5],
        effect: UpgradeEffect::DrawReward,
    },
    UpgradeDef {
        key: "muster",
        name: "Muster",
        text: "A unit that you take as a reward comes with a pawn.",
        costs: &[5],
        effect: UpgradeEffect::RewardPawn,
    },
    UpgradeDef {
        key: "curator",
        name: "Curator",
        text: "Each reward has one relic card or more.",
        costs: &[5],
        effect: UpgradeEffect::DraftRelic,
    },
];

/// The upgrades screen has a set slot for each upgrade. These are the limits of that screen.
pub const UPGRADE_SLOTS: usize = 16;
pub const UPGRADE_NAME_MAX: usize = 13;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct UpgradeId(u8);

impl UpgradeId {
    pub fn all() -> impl Iterator<Item = UpgradeId> {
        (0..UPGRADES.len() as u8).map(UpgradeId)
    }

    pub fn parse(key: &str) -> Option<UpgradeId> {
        UpgradeId::all().find(|id| id.def().key == key)
    }

    pub fn def(self) -> &'static UpgradeDef {
        &UPGRADES[self.0 as usize]
    }

    pub fn key(self) -> &'static str {
        self.def().key
    }

    pub fn max_level(self) -> u64 {
        self.def().costs.len() as u64
    }
}

// ---- Floors ----

pub struct FloorDef {
    pub name: &'static str,
    /// The level of the AI, from 1 to `chess::LEVELS`.
    pub level: usize,
    /// The total piece value of the enemy army.
    pub budget: u32,
    /// The number of boss traits.
    pub traits: usize,
    pub boss: bool,
}

const fn floor(name: &'static str, level: usize, budget: u32, traits: usize, boss: bool) -> FloorDef {
    FloorDef { name, level, budget, traits, boss }
}

/// A run has three levels of the AI: one for the floors before the first boss, one from the first
/// boss to the floor before the last boss, and one for the last boss. They are the beginner levels
/// of the engine, thus the larger army and the traits make a later floor harder. The tuning of a
/// session can change the budget, the traits, and the level of a floor (`tuning.rs`).
pub static FLOORS: [FloorDef; 8] = [
    floor("Border Patrol", 1, 5, 0, false),
    floor("Scouts", 1, 9, 0, false),
    floor("Garrison", 1, 13, 0, false),
    floor("The Warden", 2, 18, 1, true),
    floor("Cavalry", 2, 23, 0, false),
    floor("Royal Guard", 2, 28, 0, false),
    floor("Vanguard", 2, 33, 0, false),
    floor("The Black King", 3, 39, 2, true),
];

/// The floor of a number from 1 to `FLOORS.len()`.
pub fn floor_def(floor: usize) -> &'static FloorDef {
    &FLOORS[floor.clamp(1, FLOORS.len()) - 1]
}

/// The most traits that the enemy can have: the trait fan of the client has space for 2 traits.
pub const TRAITS_MAX: usize = 2;

pub fn traits_max() -> usize {
    TRAITS_MAX
}

// ---- Pieces, prices, and constants ----

/// The kinds that the player can add to the army, and the kinds of the enemy officers.
pub const RECRUIT_KINDS: [Kind; 5] = [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen];

/// The gold value of a piece. The enemy budget uses it too.
pub const fn gold_value(kind: Kind) -> u32 {
    match kind {
        Kind::Pawn => 1,
        Kind::Knight | Kind::Bishop => 3,
        Kind::Rook => 5,
        Kind::Queen => 9,
        Kind::King => 0,
    }
}

pub const fn piece_name(kind: Kind) -> &'static str {
    match kind {
        Kind::King => "King",
        Kind::Queen => "Queen",
        Kind::Rook => "Rook",
        Kind::Bishop => "Bishop",
        Kind::Knight => "Knight",
        Kind::Pawn => "Pawn",
    }
}

/// The shop price of a piece before upgrades.
pub const fn piece_price(kind: Kind) -> u64 {
    match kind {
        Kind::Pawn => 5,
        Kind::Knight | Kind::Bishop => 13,
        Kind::Rook => 20,
        Kind::Queen => 34,
        Kind::King => 0,
    }
}

pub const RELIC_PRICE: u64 = 16;
/// The relic slots of a new run. The tuning of a session can change this number (`tuning.rs`).
pub const RELIC_SLOTS: usize = 4;
/// The most relic slots of a run: the relic fan of the client has space for 10 medals.
pub const RELICS_MAX: usize = 10;
pub const REROLL_COST: u64 = 3;
pub const WIN_CROWNS: u64 = 5;
pub const ARMY_MAX: usize = 16;

/// The gold of the gold reward in the draft of a floor.
pub const fn draft_gold(floor: usize) -> u64 {
    10 + 2 * floor as u64
}

/// A recruit in the draft and the shop: its weight and the first floor that offers it.
pub struct Recruit {
    pub kind: Kind,
    pub weight: f64,
    pub min_floor: usize,
}

pub const RECRUITS: [Recruit; 5] = [
    Recruit { kind: Kind::Pawn, weight: 3.0, min_floor: 1 },
    Recruit { kind: Kind::Knight, weight: 3.0, min_floor: 1 },
    Recruit { kind: Kind::Bishop, weight: 3.0, min_floor: 1 },
    Recruit { kind: Kind::Rook, weight: 1.5, min_floor: 1 },
    Recruit { kind: Kind::Queen, weight: 0.5, min_floor: 3 },
];

/// The weight of a relic and of the gold in the draft.
pub const DRAFT_RELIC_WEIGHT: f64 = 2.0;
pub const DRAFT_GOLD_WEIGHT: f64 = 2.0;

/// A kind in an enemy army: its weight, the most pieces of the kind, and the first floor that
/// has it. The officers of an army fit on rank 8 with the king, and the pawns fit on rank 7.
pub struct EnemyKind {
    pub kind: Kind,
    pub weight: f64,
    pub cap: u32,
    pub min_floor: usize,
}

/// The kinds of an enemy army, in the order of `RECRUIT_KINDS`. The tuning of a session can
/// change these numbers (`tuning.rs`).
pub const ENEMY_KINDS: [EnemyKind; 5] = [
    EnemyKind { kind: Kind::Pawn, weight: 4.0, cap: 8, min_floor: 1 },
    EnemyKind { kind: Kind::Knight, weight: 2.0, cap: 2, min_floor: 1 },
    EnemyKind { kind: Kind::Bishop, weight: 2.0, cap: 2, min_floor: 1 },
    EnemyKind { kind: Kind::Rook, weight: 1.5, cap: 2, min_floor: 1 },
    EnemyKind { kind: Kind::Queen, weight: 1.0, cap: 1, min_floor: 5 },
];
