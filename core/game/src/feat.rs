//! The feats: the achievements of the player. A relic can have a feat as its unlock rule
//! (`content::Unlock`). A feat reads only the facts of a battle, not the engine.

use crate::chess::Kind;
use crate::protocol::names;

names! {
    /// An achievement. The name is the key in the saved data.
    #[derive(PartialOrd, Ord, Hash)]
    pub enum Feat {
        KnightMate = "knightMate",
        RookMate = "rookMate",
        CleanWin = "cleanWin",
        TwoPromotions = "twoPromotions",
        CostlyWin = "costlyWin",
        WinRun = "winRun",
    }
}

impl Feat {
    /// The condition of the feat, as the relics screen shows it.
    pub const fn text(self) -> &'static str {
        match self {
            Feat::KnightMate => "Give checkmate with a move of a knight.",
            Feat::RookMate => "Give checkmate with a move of a rook.",
            Feat::CleanWin => "Win a battle and lose no piece.",
            Feat::TwoPromotions => "Promote two pawns in a battle that you win.",
            Feat::CostlyWin => "Win a battle in which you lose three pieces or more.",
            Feat::WinRun => "Win a run.",
        }
    }

    /// True if a battle with these facts does the feat.
    pub fn met(self, facts: &BattleFacts) -> bool {
        match self {
            Feat::KnightMate => facts.mate_by == Some(Kind::Knight),
            Feat::RookMate => facts.mate_by == Some(Kind::Rook),
            Feat::CleanWin => facts.won && facts.pieces_lost == 0,
            Feat::TwoPromotions => facts.won && facts.promotions >= 2,
            Feat::CostlyWin => facts.won && facts.pieces_lost >= 3,
            Feat::WinRun => facts.won && facts.last_floor,
        }
    }
}

/// What a battle with a result gives to the feats. `Battle::facts` makes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BattleFacts {
    /// True if the battle is the battle of the last floor of a run.
    pub last_floor: bool,
    /// True if the player won the battle.
    pub won: bool,
    /// The kind of the piece that gave checkmate to the enemy. None for each other result.
    pub mate_by: Option<Kind>,
    /// The number of units that the player lost: `Battle::lost`. A unit that a relic returns after
    /// the battle is not in it, and the pawn of Conscription is not a unit.
    pub pieces_lost: u32,
    /// The number of pawns that the player promoted.
    pub promotions: u32,
}
