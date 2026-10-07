//! The feats: the achievements of the player. A relic can have a feat as its unlock rule
//! (`content::Unlock`).

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
}
