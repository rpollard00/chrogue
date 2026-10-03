//! One battle of a run. This module connects the chess engine to the run and its relics.

use crate::chess::{self, Color, Kind, Move, Outcome, Piece, Placement, Special, Square, State};
use crate::content::{self, Effect, FLOORS, RelicId};
use crate::random::{Dice, Stream};
use crate::run::{CONSCRIPT_ID, ENEMY_ID_BASE, Run, UnitId};
use crate::tuning::Tuning;

#[derive(Clone, PartialEq, Debug)]
pub struct Bonus {
    pub id: RelicId,
    pub gold: u64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct BattleReward {
    pub captures: u64,
    pub clear: u64,
    /// Extra gold from relics.
    pub bonuses: Vec<Bonus>,
}

impl BattleReward {
    pub fn total(&self) -> u64 {
        self.captures + self.clear + self.bonuses.iter().map(|b| b.gold).sum::<u64>()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BattleResult {
    pub outcome: Outcome,
    pub reward: BattleReward,
}

/// Who acts next in a battle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattlePhase {
    Player,
    Enemy,
    Over,
}

impl BattlePhase {
    pub const fn code(self) -> &'static str {
        match self {
            BattlePhase::Player => "player",
            BattlePhase::Enemy => "enemy",
            BattlePhase::Over => "over",
        }
    }
}

/// What `settle` does with the run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Next {
    Camp,
    Won,
    Lost,
}

impl Next {
    pub const fn code(self) -> &'static str {
        match self {
            Next::Camp => "camp",
            Next::Won => "won",
            Next::Lost => "lost",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Battle {
    pub state: State,
    /// The ids of the units that the enemy captured.
    pub lost: Vec<UnitId>,
    /// The ids of the captured units that return after the battle.
    pub rescued: Vec<UnitId>,
    /// The gold from captures. It can have a fraction until the battle ends.
    pub gold: f64,
    /// The kinds that each side captured: `[White, Black]`.
    pub taken: [Vec<Kind>; 2],
    pub result: Option<BattleResult>,
    pub last: Option<Move>,
    /// The number of moves that the battle played.
    pub plies: u32,
}

/// The effects of one move.
#[derive(Clone, PartialEq, Debug)]
pub struct MoveReport {
    pub mover: Piece,
    pub mv: Move,
    /// The captured piece, its square, and the gold that the capture gave.
    pub capture: Option<(Piece, Square, f64)>,
    /// The relics of the player that had an effect, in the order of the effects.
    pub relics: Vec<RelicId>,
    /// The unit that the enemy captured, and true if it returns after the battle.
    pub unit_lost: Option<(UnitId, bool)>,
}

/// The relics of a list that have an effect, in the order of the list.
fn effects(ids: &[RelicId]) -> impl Iterator<Item = (RelicId, Effect)> + '_ {
    ids.iter().filter_map(|&id| id.def().effect.map(|effect| (id, effect)))
}

fn piece(id: u16, kind: Kind, color: Color) -> Piece {
    Piece { id, kind, color, moved: false }
}

impl Battle {
    /// The pieces at the start of a battle: the army, the pieces of the relics, and the enemy.
    pub fn placements(run: &Run) -> Vec<Placement> {
        let mut pieces: Vec<Placement> = run
            .army
            .iter()
            .map(|unit| Placement { piece: piece(unit.id, unit.kind, Color::White), square: unit.home })
            .collect();
        for (_, effect) in effects(&run.relics) {
            if effect == Effect::ExtraPawn {
                // The pawn goes to the first free square of rank 2, or of rank 3 if rank 2 is full.
                // A square of an enemy piece is not free, thus a debug enemy on rank 2 or 3 does not
                // share a square.
                let taken =
                    |s: Square| pieces.iter().any(|p| p.square == s) || run.enemy.pieces.iter().any(|e| e.square == s);
                if let Some(square) = (8..24).find(|&s| !taken(s)) {
                    pieces.push(Placement { piece: piece(CONSCRIPT_ID, Kind::Pawn, Color::White), square });
                }
            }
        }
        for (i, enemy) in run.enemy.pieces.iter().enumerate() {
            pieces.push(Placement {
                piece: piece(ENEMY_ID_BASE + i as u16, enemy.kind, Color::Black),
                square: enemy.square,
            });
        }
        pieces
    }

    /// The battle of the run. An error if two pieces have one square or a side does not have
    /// exactly one king.
    pub fn new(run: &Run) -> Result<Battle, String> {
        Battle::from_placements(run, &Battle::placements(run))
    }

    /// The engine keeps the last piece of a square and accepts a side with no king or with two
    /// kings, thus the game checks the pieces before it makes a battle.
    pub fn from_placements(run: &Run, pieces: &[Placement]) -> Result<Battle, String> {
        let mut used = 0u64;
        for placement in pieces {
            if placement.square >= 64 {
                return Err(format!("Square {} is not on the board", placement.square));
            }
            let bit = 1u64 << placement.square;
            if used & bit != 0 {
                return Err(format!("Two pieces are on square {}", placement.square));
            }
            used |= bit;
        }
        for (color, side) in [(Color::White, "The army"), (Color::Black, "The enemy")] {
            let kings = pieces.iter().filter(|p| p.piece.color == color && p.piece.kind == Kind::King).count();
            if kings != 1 {
                return Err(format!("{side} must have one king, not {kings}"));
            }
        }
        let state = chess::new_state(pieces, content::rules_for(&run.relics), content::rules_for(&run.enemy.traits))?;
        Ok(Battle {
            state,
            lost: Vec::new(),
            rescued: Vec::new(),
            gold: 0.0,
            taken: [Vec::new(), Vec::new()],
            result: None,
            last: None,
            plies: 0,
        })
    }

    pub fn phase(&self) -> BattlePhase {
        match (&self.result, chess::turn(&self.state)) {
            (Some(_), _) => BattlePhase::Over,
            (None, Color::White) => BattlePhase::Player,
            (None, Color::Black) => BattlePhase::Enemy,
        }
    }

    /// The legal move of the side to move that has these squares and this promotion.
    pub fn find_move(&mut self, from: Square, to: Square, promo: Option<Kind>) -> Result<Move, FindError> {
        let candidates: Vec<Move> =
            chess::legal_moves(&mut self.state).into_iter().filter(|m| m.from == from && m.to == to).collect();
        match (candidates.as_slice(), promo) {
            ([], _) => Err(FindError::Illegal),
            ([only], None) if only.promo.is_none() => Ok(*only),
            (_, None) => Err(FindError::PromoRequired),
            (moves, Some(kind)) => moves.iter().copied().find(|m| m.promo == Some(kind)).ok_or(FindError::Illegal),
        }
    }

    /// Plays a legal move for the side to move and records its effect on the run (`playMove`).
    pub fn play(&mut self, run: &Run, mv: Move) -> MoveReport {
        let mover_color = chess::turn(&self.state);
        let mover = chess::piece_at(&self.state, mv.from).unwrap_or(piece(0, Kind::Pawn, mover_color));
        let played = chess::play(&mut self.state, mv);
        self.last = Some(mv);
        self.plies = self.plies.saturating_add(1);
        let mut report = MoveReport { mover, mv, capture: None, relics: Vec::new(), unit_lost: None };
        if let Some((captured, square)) = played.captured {
            self.taken[mover_color.index()].push(captured.kind);
            let mut gold = 0.0;
            if mover_color == Color::White {
                gold = content::gold_value(captured.kind) as f64;
                for (id, effect) in effects(&run.relics) {
                    if let Effect::CaptureGold { factor } = effect {
                        let next = gold * factor;
                        if next != gold {
                            report.relics.push(id);
                        }
                        gold = next;
                    }
                }
                self.gold += gold;
            } else if run.army.iter().any(|unit| unit.id == captured.id) {
                let rescuer = effects(&run.relics)
                    .find(|&(_, effect)| effect == Effect::RescueFirst && self.rescued.is_empty())
                    .map(|(id, _)| id);
                if let Some(id) = rescuer {
                    report.relics.push(id);
                    self.rescued.push(captured.id);
                } else {
                    self.lost.push(captured.id);
                }
                report.unit_lost = Some((captured.id, rescuer.is_some()));
            }
            report.capture = Some((captured, square, gold));
        }
        if let Some(outcome) = chess::outcome(&mut self.state) {
            let reward = self.reward_for(run, outcome);
            report.relics.extend(reward.bonuses.iter().map(|bonus| bonus.id));
            self.result = Some(BattleResult { outcome, reward });
        }
        report
    }

    fn reward_for(&self, run: &Run, outcome: Outcome) -> BattleReward {
        let mut reward = BattleReward::default();
        if outcome.winner() == Some(Color::Black) {
            return reward;
        }
        // Math.round of a number that is 0 or more.
        reward.captures = (self.gold + 0.5).floor() as u64;
        if outcome.winner().is_none() {
            return reward;
        }
        reward.clear = 3 + run.floor as u64;
        for (id, effect) in effects(&run.relics) {
            if let Effect::VictoryGold { per, max } = effect {
                let gold = (run.gold.saturating_add(reward.total()) / per).min(max);
                if gold > 0 {
                    reward.bonuses.push(Bonus { id, gold });
                }
            }
        }
        reward
    }

    /// The move of an AI level. The seed of the AI comes from the seed of the run, the floor, and
    /// the number of moves that the battle played.
    pub fn ai_move(&mut self, run: &Run, level: usize) -> Option<Move> {
        let seed = Dice::stream(run.seed, Stream::Ai, run.floor as u64, self.plies as u64).seed();
        chess::ai_move(&mut self.state, level, seed)
    }

    /// What `settle` will do.
    pub fn next(&self, run: &Run) -> Option<Next> {
        let result = self.result.as_ref()?;
        Some(match result.outcome.winner() {
            Some(Color::Black) => Next::Lost,
            Some(Color::White) if run.floor == FLOORS.len() => Next::Won,
            _ => Next::Camp,
        })
    }

    /// Applies a completed battle to the run (`settleBattle`). Returns None if the battle has no result.
    pub fn settle(&self, run: &mut Run, tuning: &Tuning) -> Option<Next> {
        let next = self.next(run)?;
        let result = self.result.as_ref()?;
        if next == Next::Lost {
            return Some(next);
        }
        run.gold = run.gold.saturating_add(result.reward.total());
        run.army.retain(|unit| !self.lost.contains(&unit.id));
        // A unit keeps the kind that it has on the board, thus a promoted pawn stays promoted.
        for (_, p) in chess::pieces(&self.state) {
            if let Some(unit) = run.army.iter_mut().find(|u| u.id == p.id) {
                unit.kind = p.kind;
            }
        }
        if next == Next::Camp {
            run.enter_camp(result.outcome.winner() == Some(Color::White), tuning);
        }
        Some(next)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FindError {
    Illegal,
    PromoRequired,
}

/// True for a move that captures: a piece on the target square, or en passant.
pub fn is_capture(state: &State, m: Move) -> bool {
    m.special == Special::EnPassant || chess::piece_at(state, m.to).is_some()
}
